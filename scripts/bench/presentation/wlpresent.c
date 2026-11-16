/*
 * wlpresent.c - bench-only LD_PRELOAD shim for libwayland-client.
 *
 * BENCH TOOLING ONLY. Never shipped, never linked into the app or the Flatpak.
 *
 * For every wl_surface.commit made by the host process it requests a
 * wp_presentation_feedback (on a private event queue, so the app's own queue
 * is never touched) and logs the commit and the presented/discarded outcome.
 *
 * Arming: active only when CROSSHOOK_BENCH=1 and CROSSHOOK_BENCH_PRESENT_DIR
 * is an absolute path. Otherwise every hook is a pure pass-through. There is
 * no constructor and no thread at load time; everything starts lazily on the
 * first wl_surface.commit. That first commit blocks for a few roundtrips on the
 * private queue (registry bind, clock_id). After a fork() the child is a pure
 * pass-through (pthread_atfork, registered at init).
 *
 * Log: <CROSSHOOK_BENCH_PRESENT_DIR>/wlpresent-<pid>.log, created 0600 with
 * O_EXCL|O_NOFOLLOW. Format "wlpresent v1": one tab-separated record per
 * line, first field is the record type, all <ns> are CLOCK_MONOTONIC ns:
 *   H version=1 pid=.. comm=.. clock_id=.. pres_version=..
 *     WEBKIT_DISABLE_DMABUF_RENDERER=<v|-> GDK_BACKEND=<v|->   (header)
 *   S <ns> <surface_id> toplevel        wl_surface became an xdg toplevel
 *                                       (ns is the time of the request; records
 *                                       made before init are flushed after H)
 *   S <ns> <surface_id> destroyed       that toplevel wl_surface was destroyed, so
 *                                       its id may be reused by another surface
 *   C <commit_ns> <surface_id> <feedback_seq> <has_buffer>
 *                                       has_buffer=1 iff a non-NULL
 *                                       wl_surface.attach preceded this commit
 *   P <recv_ns> <feedback_seq> <presented_ns> <refresh_ns> <msc> <flags>
 *                                       presented_ns is in the clock_id domain
 *   D <recv_ns> <feedback_seq>          feedback discarded
 *   X <ns> <reason>                     no_presentation_global | bind_failed |
 *                                       log_cap (500,000 records, then silent)
 *   K|M <ns> enter|leave <surface_id>   wl_keyboard / wl_pointer focus  } listener wrappers
 *   G <ns> <surface_id> <w> <h> <activated 0|1>  xdg_toplevel.configure  } (see wlfocus.c)
 */
#define _GNU_SOURCE
#include <wayland-version.h>
#if WAYLAND_VERSION_MAJOR < 1 || (WAYLAND_VERSION_MAJOR == 1 && WAYLAND_VERSION_MINOR < 24)
#error "wlpresent needs libwayland-client >= 1.24 (wl_proxy_get_interface)"
#endif
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "presentation-time-client-protocol.h"
#include "wlpresent.h"

#ifndef WL_CLOSURE_MAX_ARGS
#define WL_CLOSURE_MAX_ARGS 20
#endif
/* The longest non-header record is a P line (about 108 bytes); the header is under 400.
 * The cap keeps a full log below the parser's 64 MiB MAX_LOG_BYTES. */
#define LOG_CAP 500000UL
#define LOG_RECORD_MAX_BYTES 128UL
_Static_assert(LOG_CAP * LOG_RECORD_MAX_BYTES + 4096UL < 64UL * 1024UL * 1024UL,
               "a capped log must stay below the parser's MAX_LOG_BYTES");
#define MAX_SURFACES 512

enum { OP_DESTROY = 0, OP_ATTACH = 1, OP_GET_TOPLEVEL = 1, OP_GET_XDG_SURFACE = 2, OP_COMMIT = 6 };

typedef struct wl_proxy *(*array_flags_fn)(struct wl_proxy *, uint32_t, const struct wl_interface *,
                                           uint32_t, uint32_t, union wl_argument *);

struct fb_ctx { uint64_t seq; };

static pthread_once_t g_arm_once = PTHREAD_ONCE_INIT;
static int g_armed;
static char g_dir[PATH_MAX - 64];

static pthread_mutex_t g_init_lock = PTHREAD_MUTEX_INITIALIZER;
pthread_mutex_t g_tab_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t g_log_lock = PTHREAD_MUTEX_INITIALIZER;
int g_state; /* 0 not initialized, 1 ready, -1 pass-through (disabled or failed) */
static __thread int in_hook;

static struct surf g_tab[MAX_SURFACES];
static struct wl_event_queue *g_queue;
static struct wp_presentation *g_pres;
static struct wl_display *g_display;
static int g_clock_id = -1;
static uint32_t g_pres_version;
static uint64_t g_seq;
static int g_log_fd = -1, g_log_capped;
static int g_atfork_registered;
static unsigned long g_log_count;

uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static void write_all(int fd, const char *buf, size_t len) {
    while (len > 0) {
        ssize_t n = write(fd, buf, len);
        if (n < 0) {
            if (errno == EINTR) continue;
            return;
        }
        buf += n;
        len -= (size_t)n;
    }
}

/* One write(2) per record; the mutex keeps records from interleaving. */
void rec(const char *fmt, ...) {
    char buf[1024];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof buf - 1, fmt, ap);
    va_end(ap);
    if (n <= 0) return;
    if ((size_t)n > sizeof buf - 2) n = (int)sizeof buf - 2;
    buf[n++] = '\n';
    pthread_mutex_lock(&g_log_lock);
    if (g_log_fd >= 0 && !g_log_capped) {
        if (g_log_count >= LOG_CAP) {
            char cap[64];
            int m = snprintf(cap, sizeof cap, "X\t%llu\tlog_cap\n", ULL(now_ns()));
            write_all(g_log_fd, cap, (size_t)m);
            g_log_capped = 1;
        } else {
            write_all(g_log_fd, buf, (size_t)n);
            g_log_count++;
        }
    }
    pthread_mutex_unlock(&g_log_lock);
}

/* ---- surface table (caller holds g_tab_lock) ---- */

static struct surf *surf_find(uint32_t wl_id, int create) {
    struct surf *freeslot = NULL;
    for (int i = 0; i < MAX_SURFACES; i++) {
        if (g_tab[i].wl_id == wl_id) return &g_tab[i];
        if (!freeslot && g_tab[i].wl_id == 0) freeslot = &g_tab[i];
    }
    if (create && freeslot) {
        memset(freeslot, 0, sizeof *freeslot);
        freeslot->wl_id = wl_id;
    }
    return create ? freeslot : NULL;
}

static struct surf *surf_find_xdg(uint32_t xdg_id) {
    for (int i = 0; i < MAX_SURFACES; i++)
        if (g_tab[i].wl_id && g_tab[i].xdg_id == xdg_id) return &g_tab[i];
    return NULL;
}

struct surf *surf_find_toplevel(uint32_t toplevel_id) {
    for (int i = 0; toplevel_id && i < MAX_SURFACES; i++)
        if (g_tab[i].wl_id && g_tab[i].toplevel_id == toplevel_id) return &g_tab[i];
    return NULL;
}

/* ---- presentation feedback ---- */

static void fb_sync_output(void *d UNUSED, struct wp_presentation_feedback *f UNUSED,
                           struct wl_output *o UNUSED) {}

static void fb_presented(void *d, struct wp_presentation_feedback *f, uint32_t hi, uint32_t lo,
                         uint32_t nsec, uint32_t refresh, uint32_t seq_hi, uint32_t seq_lo,
                         uint32_t flags) {
    struct fb_ctx *c = d;
    uint64_t recv = now_ns();
    uint64_t presented = (((uint64_t)hi << 32) | lo) * 1000000000ULL + nsec;
    uint64_t msc = ((uint64_t)seq_hi << 32) | seq_lo;
    rec("P\t%llu\t%llu\t%llu\t%u\t%llu\t%u", ULL(recv), ULL(c->seq), ULL(presented), refresh,
        ULL(msc), flags);
    wp_presentation_feedback_destroy(f);
    free(c);
}

static void fb_discarded(void *d, struct wp_presentation_feedback *f) {
    struct fb_ctx *c = d;
    rec("D\t%llu\t%llu", ULL(now_ns()), ULL(c->seq));
    wp_presentation_feedback_destroy(f);
    free(c);
}

static const struct wp_presentation_feedback_listener fb_listener = {fb_sync_output, fb_presented,
                                                                       fb_discarded};

static void pres_clock_id(void *d UNUSED, struct wp_presentation *p UNUSED, uint32_t id) {
    g_clock_id = (int)id;
}
static const struct wp_presentation_listener pres_listener = {pres_clock_id};

static void reg_global(void *d UNUSED, struct wl_registry *r, uint32_t name, const char *iface,
                       uint32_t ver) {
    if (g_pres || strcmp(iface, "wp_presentation") != 0) return;
    g_pres_version = ver < 2 ? ver : 2;
    g_pres = wl_registry_bind(r, name, &wp_presentation_interface, g_pres_version);
    if (g_pres) wp_presentation_add_listener(g_pres, &pres_listener, NULL);
}
static void reg_remove(void *d UNUSED, struct wl_registry *r UNUSED, uint32_t n UNUSED) {}
static const struct wl_registry_listener reg_listener = {reg_global, reg_remove};

static void *pump(void *arg UNUSED) {
    while (wl_display_dispatch_queue(g_display, g_queue) != -1) {
    }
    return NULL;
}

/* ---- lazy init ---- */

static void sanitized_env(const char *name, char *out, size_t cap) {
    const char *v = getenv(name);
    if (!v || !*v) v = "-";
    size_t i = 0;
    for (; v[i] && i + 1 < cap; i++) out[i] = (unsigned char)v[i] < 0x20 ? '_' : v[i];
    out[i] = '\0';
}

static void read_comm(char *out, size_t cap) {
    snprintf(out, cap, "-");
    int fd = open("/proc/self/comm", O_RDONLY | O_CLOEXEC);
    if (fd < 0) return;
    ssize_t n = read(fd, out, cap - 1);
    close(fd);
    if (n <= 0) {
        snprintf(out, cap, "-");
        return;
    }
    if (out[n - 1] == '\n') n--;
    out[n] = '\0';
    for (ssize_t i = 0; i < n; i++)
        if ((unsigned char)out[i] < 0x20) out[i] = '_';
}

/* Drops everything the shim holds on the Wayland connection (init failure path). */
static void release_wayland(struct wl_display *wrap, struct wl_registry *reg) {
    if (wrap) wl_proxy_wrapper_destroy(wrap);
    if (reg) wl_registry_destroy(reg);
    if (g_pres) {
        wp_presentation_destroy(g_pres);
        g_pres = NULL;
    }
    if (g_queue) {
        wl_event_queue_destroy(g_queue);
        g_queue = NULL;
    }
}

static void close_log(void) {
    pthread_mutex_lock(&g_log_lock);
    if (g_log_fd >= 0) close(g_log_fd);
    g_log_fd = -1;
    pthread_mutex_unlock(&g_log_lock);
}

/* In a forked child the pump thread is gone and the connection belongs to the parent,
 * so the child must never touch either. No locks here: another thread of the parent
 * may have held one at fork time. */
static void child_handler(void) {
    __atomic_store_n(&g_state, -1, __ATOMIC_RELEASE);
    if (g_log_fd >= 0) close(g_log_fd);
    g_log_fd = -1;
}

static void write_header(void) {
    char comm[64], dmabuf[64], gdk[64];
    read_comm(comm, sizeof comm);
    sanitized_env("WEBKIT_DISABLE_DMABUF_RENDERER", dmabuf, sizeof dmabuf);
    sanitized_env("GDK_BACKEND", gdk, sizeof gdk);
    rec("H\tversion=1\tpid=%ld\tcomm=%s\tclock_id=%d\tpres_version=%u\t"
        "WEBKIT_DISABLE_DMABUF_RENDERER=%s\tGDK_BACKEND=%s",
        (long)getpid(), comm, g_clock_id, g_pres_version, dmabuf, gdk);
}

static int start_pump(void) {
    sigset_t all, old;
    pthread_t th;
    sigfillset(&all);
    pthread_sigmask(SIG_BLOCK, &all, &old); /* the pump must never take app signals */
    int rc = pthread_create(&th, NULL, pump, NULL);
    pthread_sigmask(SIG_SETMASK, &old, NULL);
    if (rc != 0) return -1;
    pthread_detach(th);
    return 0;
}

/* Runs once under g_init_lock; leaves g_state at 1 or -1. */
static void do_init(struct wl_proxy *any) {
    char path[PATH_MAX];
    struct wl_display *wrap = NULL;
    struct wl_registry *reg = NULL;
    int n = snprintf(path, sizeof path, "%s/wlpresent-%ld.log", g_dir, (long)getpid());
    if (n <= 0 || (size_t)n >= sizeof path) goto off;
    g_log_fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_APPEND | O_CLOEXEC, 0600);
    if (g_log_fd < 0) goto off;
    if (!g_atfork_registered) {
        pthread_atfork(NULL, NULL, child_handler);
        g_atfork_registered = 1;
    }

    g_display = wl_proxy_get_display(any);
    g_queue = g_display ? wl_display_create_queue(g_display) : NULL;
    wrap = g_queue ? wl_proxy_create_wrapper(g_display) : NULL;
    if (!wrap) goto bind_failed;
    wl_proxy_set_queue((struct wl_proxy *)wrap, g_queue);
    reg = wl_display_get_registry(wrap);
    if (!reg) goto bind_failed;
    wl_registry_add_listener(reg, &reg_listener, NULL);
    if (wl_display_roundtrip_queue(g_display, g_queue) < 0) goto bind_failed;
    if (g_pres && wl_display_roundtrip_queue(g_display, g_queue) < 0) goto bind_failed; /* clock_id */
    wl_proxy_wrapper_destroy(wrap);
    wrap = NULL;
    wl_registry_destroy(reg);
    reg = NULL;

    write_header();
    if (!g_pres) {
        rec("X\t%llu\tno_presentation_global", ULL(now_ns()));
        release_wayland(NULL, NULL);
        goto off;
    }
    if (start_pump() != 0) goto bind_failed;

    /* Flush toplevel marks made before the log existed, then go live. */
    pthread_mutex_lock(&g_tab_lock);
    wlfocus_flush_pending();
    for (int i = 0; i < MAX_SURFACES; i++) {
        if (g_tab[i].wl_id && g_tab[i].toplevel)
            rec("S\t%llu\t%u\ttoplevel", ULL(g_tab[i].toplevel_ns), g_tab[i].wl_id);
        struct surf *s = &g_tab[i];
        if (s->wl_id && s->cfg_pending) {
            rec_geometry(s->cfg_ns, s->wl_id, s->cfg_w, s->cfg_h, s->cfg_activated);
            s->cfg_pending = 0;
        }
    }
    __atomic_store_n(&g_state, 1, __ATOMIC_RELEASE);
    pthread_mutex_unlock(&g_tab_lock);
    return;

bind_failed:
    release_wayland(wrap, reg);
    rec("X\t%llu\tbind_failed", ULL(now_ns()));
off:
    close_log();
    __atomic_store_n(&g_state, -1, __ATOMIC_RELEASE);
}

static void init_once(struct wl_proxy *any) {
    /* trylock: a thread that loses the race just skips tracking this commit. */
    if (pthread_mutex_trylock(&g_init_lock) != 0) return;
    if (__atomic_load_n(&g_state, __ATOMIC_ACQUIRE) == 0) do_init(any);
    pthread_mutex_unlock(&g_init_lock);
}

/* ---- request tracking ---- */

static void arm_once(void) {
    const char *e = getenv("CROSSHOOK_BENCH");
    const char *d = getenv("CROSSHOOK_BENCH_PRESENT_DIR");
    if (!e || strcmp(e, "1") != 0 || !d || d[0] != '/' || strlen(d) >= sizeof g_dir) return;
    strcpy(g_dir, d);
    g_armed = 1;
}

static int is_class(struct wl_proxy *p, const char *name) {
    return strcmp(wl_proxy_get_class(p), name) == 0;
}

static void on_commit(struct wl_proxy *p) {
    init_once(p);
    if (__atomic_load_n(&g_state, __ATOMIC_ACQUIRE) != 1) return;
    uint64_t t = now_ns();
    uint32_t sid = wl_proxy_get_id(p);
    int has_buffer = 0;
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find(sid, 0);
    if (s) {
        has_buffer = s->has_buffer;
        s->has_buffer = 0;
    }
    pthread_mutex_unlock(&g_tab_lock);
    struct fb_ctx *c = calloc(1, sizeof *c);
    if (!c) return;
    c->seq = __atomic_fetch_add(&g_seq, 1, __ATOMIC_RELAXED);
    struct wp_presentation_feedback *f = wp_presentation_feedback(g_pres, (struct wl_surface *)p);
    if (!f) {
        free(c);
        return;
    }
    wp_presentation_feedback_add_listener(f, &fb_listener, c);
    rec("C\t%llu\t%u\t%llu\t%d", ULL(t), sid, ULL(c->seq), has_buffer);
}

static void on_attach(struct wl_proxy *p, union wl_argument *args) {
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find(wl_proxy_get_id(p), 1);
    if (s) s->has_buffer = args[0].o != NULL;
    pthread_mutex_unlock(&g_tab_lock);
}

static void on_toplevel(struct wl_proxy *p) {
    uint64_t t = now_ns();
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find_xdg(wl_proxy_get_id(p));
    if (s) {
        s->toplevel = 1;
        s->toplevel_ns = t;
        if (__atomic_load_n(&g_state, __ATOMIC_ACQUIRE) == 1)
            rec("S\t%llu\t%u\ttoplevel", ULL(t), s->wl_id);
    }
    pthread_mutex_unlock(&g_tab_lock);
}

static void on_destroy(struct wl_proxy *p, int is_wl_surface) {
    uint64_t t = now_ns();
    uint32_t id = wl_proxy_get_id(p);
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = is_wl_surface ? surf_find(id, 0) : surf_find_xdg(id);
    if (s && is_wl_surface) {
        /* The server may reuse the id; tell the parser the toplevel role ends here. */
        if (s->toplevel && __atomic_load_n(&g_state, __ATOMIC_ACQUIRE) == 1)
            rec("S\t%llu\t%u\tdestroyed", ULL(t), id);
        memset(s, 0, sizeof *s);
    } else if (s) {
        s->xdg_id = 0;
        s->toplevel_id = 0;
    }
    pthread_mutex_unlock(&g_tab_lock);
}

static void on_toplevel_created(struct wl_proxy *xdg_surface, struct wl_proxy *toplevel) {
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find_xdg(wl_proxy_get_id(xdg_surface));
    if (s) s->toplevel_id = wl_proxy_get_id(toplevel);
    pthread_mutex_unlock(&g_tab_lock);
}

static void on_toplevel_destroy(struct wl_proxy *toplevel) {
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find_toplevel(wl_proxy_get_id(toplevel));
    if (s) s->toplevel_id = 0;
    pthread_mutex_unlock(&g_tab_lock);
}

static void before(struct wl_proxy *p, uint32_t op, union wl_argument *args) {
    if (op != OP_DESTROY && op != OP_ATTACH && op != OP_COMMIT) return;
    if (is_class(p, "wl_surface")) {
        if (op == OP_COMMIT) on_commit(p);
        else if (op == OP_ATTACH) on_attach(p, args);
        else on_destroy(p, 1);
    } else if (is_class(p, "xdg_surface")) {
        if (op == OP_GET_TOPLEVEL) on_toplevel(p);
        else if (op == OP_DESTROY) on_destroy(p, 0);
    } else if (op == OP_DESTROY && is_class(p, "xdg_toplevel")) {
        on_toplevel_destroy(p);
    }
}

static void after(struct wl_proxy *p, uint32_t op, union wl_argument *args, struct wl_proxy *ret) {
    if (!ret) return;
    if (op == OP_GET_TOPLEVEL && is_class(p, "xdg_surface")) {
        on_toplevel_created(p, ret);
        return;
    }
    if (op != OP_GET_XDG_SURFACE || !is_class(p, "xdg_wm_base") || !args[1].o) return;
    uint32_t sid = wl_proxy_get_id((struct wl_proxy *)args[1].o);
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find(sid, 1);
    if (s) s->xdg_id = wl_proxy_get_id(ret);
    pthread_mutex_unlock(&g_tab_lock);
}

/* True while hooks may wrap/log: armed, not re-entered, not failed (g_state 0 still counts). */
int wlp_tracking(void) {
    pthread_once(&g_arm_once, arm_once);
    return g_armed && !in_hook && __atomic_load_n(&g_state, __ATOMIC_ACQUIRE) >= 0;
}

static struct wl_proxy *forward_request(struct wl_proxy *proxy, uint32_t opcode,
                                        const struct wl_interface *interface, uint32_t version,
                                        uint32_t flags, union wl_argument *args) {
    static array_flags_fn real;
    array_flags_fn fn = __atomic_load_n(&real, __ATOMIC_ACQUIRE);
    if (!fn) {
        fn = (array_flags_fn)dlsym(RTLD_NEXT, "wl_proxy_marshal_array_flags");
        if (!fn) abort();
        __atomic_store_n(&real, fn, __ATOMIC_RELEASE);
    }
    /* The guard also covers the real call, so any re-entry is a plain pass-through. */
    if (!wlp_tracking()) return fn(proxy, opcode, interface, version, flags, args);
    in_hook = 1;
    before(proxy, opcode, args);
    struct wl_proxy *ret = fn(proxy, opcode, interface, version, flags, args);
    after(proxy, opcode, args, ret);
    in_hook = 0;
    return ret;
}

/* A destructor request destroys the proxy inside libwayland: forget the proxy before it, free
 * our listener copy after it (libwayland may still read the table during the destroy). */
static struct wl_proxy *forward(struct wl_proxy *proxy, uint32_t opcode,
                                const struct wl_interface *interface, uint32_t version,
                                uint32_t flags, union wl_argument *args) {
    wl_fn *copy = (flags & WL_MARSHAL_FLAG_DESTROY) ? wlfocus_detach(proxy) : NULL;
    struct wl_proxy *ret = forward_request(proxy, opcode, interface, version, flags, args);
    wlfocus_free(copy);
    return ret;
}

/* ---- exported hooks ---- */

struct wl_proxy *wl_proxy_marshal_array_flags(struct wl_proxy *proxy, uint32_t opcode,
                                              const struct wl_interface *interface,
                                              uint32_t version, uint32_t flags,
                                              union wl_argument *args) {
    return forward(proxy, opcode, interface, version, flags, args);
}

/* There is no va_list forwarding variant, so every call is packed into an
 * argument array using the request's signature and sent via array_flags. */
struct wl_proxy *wl_proxy_marshal_flags(struct wl_proxy *proxy, uint32_t opcode,
                                        const struct wl_interface *interface, uint32_t version,
                                        uint32_t flags, ...) {
    union wl_argument args[WL_CLOSURE_MAX_ARGS];
    const struct wl_interface *pi = wl_proxy_get_interface(proxy);
    if (!pi || opcode >= (uint32_t)pi->method_count) abort();
    memset(args, 0, sizeof args);
    va_list ap;
    va_start(ap, flags);
    int n = 0;
    for (const char *s = pi->methods[opcode].signature; *s && n < WL_CLOSURE_MAX_ARGS; s++) {
        switch (*s) {
        case 'i': args[n++].i = va_arg(ap, int32_t); break;
        case 'u': args[n++].u = va_arg(ap, uint32_t); break;
        case 'f': args[n++].f = va_arg(ap, wl_fixed_t); break;
        case 's': args[n++].s = va_arg(ap, const char *); break;
        case 'o': args[n++].o = va_arg(ap, struct wl_object *); break;
        case 'n': (void)va_arg(ap, struct wl_object *); args[n++].o = NULL; break;
        case 'a': args[n++].a = va_arg(ap, struct wl_array *); break;
        case 'h': args[n++].h = va_arg(ap, int32_t); break;
        default: break; /* '?' nullable marker and version digits carry no argument */
        }
    }
    va_end(ap);
    return forward(proxy, opcode, interface, version, flags, args);
}
