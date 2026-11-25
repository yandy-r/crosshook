/*
 * wlfocus.c - focus and geometry records for the wlpresent shim.
 *
 * BENCH TOOLING ONLY. wl_proxy_add_listener (hooked in wlpresent.c) hands every
 * wl_keyboard, wl_pointer and xdg_toplevel listener table to wlfocus_wrap(), which
 * returns a copy whose enter/leave/configure slots log a record and then call the
 * client's original callback with unchanged arguments. All other slots are copied as is.
 *
 *   K <ns> enter|leave <surface_wl_id>     wl_keyboard.enter / leave
 *   M <ns> enter|leave <surface_wl_id>     wl_pointer.enter / leave
 *   G <ns> <surface_wl_id> <w> <h> <activated 0|1>
 *                                          xdg_toplevel.configure; activated is state 4
 *
 *   X <ns> wrap_failed                     a listener table could not be wrapped (map full, out
 *                                          of memory, unknown bound version or a stale entry for
 *                                          a reused address): that proxy's events go unrecorded
 *
 * A NULL surface logs id 0. Configures (the latest per surface) and a wrap_failed that arrive
 * before the log exists are kept and flushed after the header.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>
#include "wlfocus_slots.h"
#include "wlpresent.h"

#define MAX_WRAPPED 256
#define XDG_TOPLEVEL_STATE_ACTIVATED 4

struct slot_def {
    unsigned idx;
    wl_fn fn;
};

struct wrapped {
    struct wl_proxy *proxy;
    wl_fn *orig, *copy;
};

static pthread_mutex_t g_wrap_lock = PTHREAD_MUTEX_INITIALIZER;
static struct wrapped g_wrapped[MAX_WRAPPED];

/* A wrap failure before the log is live (g_tab_lock guards both): rec() would drop it, so it is
 * kept like a pending configure and flushed after the header. */
static int g_wrap_failed_pending;
static uint64_t g_wrap_failed_ns;

/* The client's original callback for a slot of a wrapped proxy, or NULL. */
static wl_fn orig_slot(void *proxy, unsigned idx) {
    wl_fn f = NULL;
    pthread_mutex_lock(&g_wrap_lock);
    for (int i = 0; i < MAX_WRAPPED; i++)
        if (g_wrapped[i].proxy == (struct wl_proxy *)proxy) {
            f = g_wrapped[i].orig[idx];
            break;
        }
    pthread_mutex_unlock(&g_wrap_lock);
    return f;
}

static uint32_t surface_id(struct wl_surface *s) {
    return s ? wl_proxy_get_id((struct wl_proxy *)s) : 0;
}

void rec_geometry(uint64_t ns, uint32_t surface_id_, int w, int h, int activated) {
    rec("G\t%llu\t%u\t%d\t%d\t%d", ULL(ns), surface_id_, w, h, activated);
}

/* ---- wl_keyboard ---- */

static void kb_enter(void *data, struct wl_keyboard *kb, uint32_t serial, struct wl_surface *surface,
                     struct wl_array *keys) {
    typedef void (*fn_t)(void *, struct wl_keyboard *, uint32_t, struct wl_surface *, struct wl_array *);
    rec("K\t%llu\tenter\t%u", ULL(now_ns()), surface_id(surface));
    fn_t f = (fn_t)orig_slot(kb, KB_ENTER);
    if (f) f(data, kb, serial, surface, keys);
}

static void kb_leave(void *data, struct wl_keyboard *kb, uint32_t serial, struct wl_surface *surface) {
    typedef void (*fn_t)(void *, struct wl_keyboard *, uint32_t, struct wl_surface *);
    rec("K\t%llu\tleave\t%u", ULL(now_ns()), surface_id(surface));
    fn_t f = (fn_t)orig_slot(kb, KB_LEAVE);
    if (f) f(data, kb, serial, surface);
}

/* ---- wl_pointer ---- */

static void ptr_enter(void *data, struct wl_pointer *ptr, uint32_t serial, struct wl_surface *surface,
                      wl_fixed_t x, wl_fixed_t y) {
    typedef void (*fn_t)(void *, struct wl_pointer *, uint32_t, struct wl_surface *, wl_fixed_t,
                         wl_fixed_t);
    rec("M\t%llu\tenter\t%u", ULL(now_ns()), surface_id(surface));
    fn_t f = (fn_t)orig_slot(ptr, PTR_ENTER);
    if (f) f(data, ptr, serial, surface, x, y);
}

static void ptr_leave(void *data, struct wl_pointer *ptr, uint32_t serial, struct wl_surface *surface) {
    typedef void (*fn_t)(void *, struct wl_pointer *, uint32_t, struct wl_surface *);
    rec("M\t%llu\tleave\t%u", ULL(now_ns()), surface_id(surface));
    fn_t f = (fn_t)orig_slot(ptr, PTR_LEAVE);
    if (f) f(data, ptr, serial, surface);
}

/* ---- xdg_toplevel (struct xdg_toplevel is opaque here: no xdg-shell header is built) ---- */

static void log_configure(struct wl_proxy *toplevel, int32_t w, int32_t h, struct wl_array *states) {
    if (__atomic_load_n(&g_state, __ATOMIC_ACQUIRE) < 0) return;
    uint64_t t = now_ns();
    int activated = 0;
    if (states) {
        uint32_t *state;
        wl_array_for_each(state, states) if (*state == XDG_TOPLEVEL_STATE_ACTIVATED) activated = 1;
    }
    pthread_mutex_lock(&g_tab_lock);
    struct surf *s = surf_find_toplevel(wl_proxy_get_id(toplevel));
    if (s && __atomic_load_n(&g_state, __ATOMIC_ACQUIRE) == 1) {
        rec_geometry(t, s->wl_id, w, h, activated);
    } else if (s) {
        s->cfg_pending = 1;
        s->cfg_ns = t;
        s->cfg_w = w;
        s->cfg_h = h;
        s->cfg_activated = activated;
    }
    pthread_mutex_unlock(&g_tab_lock);
}

static void tl_configure(void *data, struct wl_proxy *toplevel, int32_t w, int32_t h,
                         struct wl_array *states) {
    typedef void (*fn_t)(void *, struct wl_proxy *, int32_t, int32_t, struct wl_array *);
    log_configure(toplevel, w, h, states);
    fn_t f = (fn_t)orig_slot(toplevel, TL_CONFIGURE);
    if (f) f(data, toplevel, w, h, states);
}

/* ---- listener table copies ---- */

static const struct slot_def kb_slots[] = {{KB_ENTER, (wl_fn)kb_enter}, {KB_LEAVE, (wl_fn)kb_leave}};
static const struct slot_def ptr_slots[] = {{PTR_ENTER, (wl_fn)ptr_enter}, {PTR_LEAVE, (wl_fn)ptr_leave}};
static const struct slot_def tl_slots[] = {{TL_CONFIGURE, (wl_fn)tl_configure}};

static const struct slot_def *slots_for(const char *cls, size_t *count) {
    if (strcmp(cls, "wl_keyboard") == 0) {
        *count = sizeof kb_slots / sizeof kb_slots[0];
        return kb_slots;
    }
    if (strcmp(cls, "wl_pointer") == 0) {
        *count = sizeof ptr_slots / sizeof ptr_slots[0];
        return ptr_slots;
    }
    if (strcmp(cls, "xdg_toplevel") == 0) {
        *count = sizeof tl_slots / sizeof tl_slots[0];
        return tl_slots;
    }
    return NULL;
}

/* Records that a proxy's events will go unrecorded, or queues the record until the header. */
static void note_wrap_failed(void) {
    uint64_t t = now_ns();
    pthread_mutex_lock(&g_tab_lock);
    if (__atomic_load_n(&g_state, __ATOMIC_ACQUIRE) == 1) {
        rec("X\t%llu\twrap_failed", ULL(t));
    } else if (!g_wrap_failed_pending) {
        g_wrap_failed_pending = 1;
        g_wrap_failed_ns = t;
    }
    pthread_mutex_unlock(&g_tab_lock);
}

/* Called by do_init with g_tab_lock held, right after the header. */
void wlfocus_flush_pending(void) {
    if (!g_wrap_failed_pending) return;
    rec("X\t%llu\twrap_failed", ULL(g_wrap_failed_ns));
    g_wrap_failed_pending = 0;
}

wl_fn *wlfocus_wrap(struct wl_proxy *proxy, wl_fn *impl) {
    size_t count = 0;
    const struct slot_def *defs = impl ? slots_for(wl_proxy_get_class(proxy), &count) : NULL;
    if (!defs) return impl;
    const struct wl_interface *iface = wl_proxy_get_interface(proxy);
    if (!iface) return impl;
    for (size_t i = 0; i < count; i++)
        if (defs[i].idx >= (unsigned)iface->event_count) return impl;
    uint32_t version = wl_proxy_get_version(proxy);
    if (version == 0) { /* unknown bound version: no slot can be proven safe to read */
        note_wrap_failed();
        return impl;
    }
    /* The client's table is const and client-owned; the copy is ours, sized from the interface.
     * Slots beyond the client's table stay NULL and are never wrapped. */
    wl_fn *copy = calloc((size_t)iface->event_count, sizeof *copy);
    if (copy) {
        for (int i = 0; i < iface->event_count; i++)
            if (slot_provided(iface, (unsigned)i, version)) copy[i] = impl[i];
        for (size_t i = 0; i < count; i++)
            if (slot_provided(iface, defs[i].idx, version)) copy[defs[i].idx] = defs[i].fn;
    }

    struct wrapped *slot = NULL;
    int duplicate = 0, benign = 0;
    if (copy) {
        pthread_mutex_lock(&g_wrap_lock);
        for (int i = 0; i < MAX_WRAPPED; i++) {
            if (g_wrapped[i].proxy == proxy) {
                /* Our copy still installed on the live proxy: this is a second add_listener,
                 * which libwayland rejects, so stay out. Anything else is a stale entry for a
                 * reused address (the proxy went away without our destroy hook). */
                duplicate = 1;
                benign = wl_proxy_get_listener(proxy) == (const void *)g_wrapped[i].copy;
                break;
            }
            if (!slot && !g_wrapped[i].proxy) slot = &g_wrapped[i];
        }
        if (slot && !duplicate) *slot = (struct wrapped){proxy, impl, copy};
        pthread_mutex_unlock(&g_wrap_lock);
        if (slot && !duplicate) return copy;
        free(copy);
        if (benign) return impl;
    }
    note_wrap_failed();
    return impl;
}

wl_fn *wlfocus_detach(struct wl_proxy *proxy) {
    wl_fn *copy = NULL;
    pthread_mutex_lock(&g_wrap_lock);
    for (int i = 0; i < MAX_WRAPPED; i++)
        if (g_wrapped[i].proxy == proxy) {
            copy = g_wrapped[i].copy;
            memset(&g_wrapped[i], 0, sizeof g_wrapped[i]);
            break;
        }
    pthread_mutex_unlock(&g_wrap_lock);
    return copy;
}

void wlfocus_free(wl_fn *copy) {
    free(copy);
}

/* ---- wl_proxy_add_dispatcher (wayland-rs clients: winit/iced) ----
 * SPIKE BRANCH ONLY (YAN-773). wayland-rs never calls wl_proxy_add_listener; it installs one
 * dispatcher per proxy and keeps `dispatcher_data` as its ownership tag (checked through
 * wl_proxy_get_listener). The wrapper keeps that tag unchanged, logs the same K/M/G records
 * and then calls the original dispatcher with unchanged arguments. */

enum { DISP_KB = 1, DISP_PTR = 2, DISP_TL = 3 };

struct wrapped_disp {
    struct wl_proxy *proxy;
    wl_dispatcher_func_t fn;
    int cls;
};

static struct wrapped_disp g_disp[MAX_WRAPPED];

static int disp_class(const char *cls) {
    if (strcmp(cls, "wl_keyboard") == 0) return DISP_KB;
    if (strcmp(cls, "wl_pointer") == 0) return DISP_PTR;
    if (strcmp(cls, "xdg_toplevel") == 0) return DISP_TL;
    return 0;
}

static int disp_wrap(const void *data, void *target, uint32_t opcode, const struct wl_message *msg,
                     union wl_argument *args) {
    wl_dispatcher_func_t fn = NULL;
    int cls = 0;
    pthread_mutex_lock(&g_wrap_lock);
    for (int i = 0; i < MAX_WRAPPED; i++)
        if (g_disp[i].proxy == (struct wl_proxy *)target) {
            fn = g_disp[i].fn;
            cls = g_disp[i].cls;
            break;
        }
    pthread_mutex_unlock(&g_wrap_lock);
    if (cls == DISP_KB && (opcode == KB_ENTER || opcode == KB_LEAVE))
        rec("K\t%llu\t%s\t%u", ULL(now_ns()), opcode == KB_ENTER ? "enter" : "leave",
            surface_id((struct wl_surface *)args[1].o));
    else if (cls == DISP_PTR && (opcode == PTR_ENTER || opcode == PTR_LEAVE))
        rec("M\t%llu\t%s\t%u", ULL(now_ns()), opcode == PTR_ENTER ? "enter" : "leave",
            surface_id((struct wl_surface *)args[1].o));
    else if (cls == DISP_TL && opcode == TL_CONFIGURE)
        log_configure((struct wl_proxy *)target, args[0].i, args[1].i, args[2].a);
    return fn ? fn(data, target, opcode, msg, args) : -1;
}

static void disp_detach(struct wl_proxy *proxy) {
    pthread_mutex_lock(&g_wrap_lock);
    for (int i = 0; i < MAX_WRAPPED; i++)
        if (g_disp[i].proxy == proxy) memset(&g_disp[i], 0, sizeof g_disp[i]);
    pthread_mutex_unlock(&g_wrap_lock);
}

int wl_proxy_add_dispatcher(struct wl_proxy *proxy, wl_dispatcher_func_t dispatcher,
                            const void *dispatcher_data, void *data) {
    typedef int (*add_fn)(struct wl_proxy *, wl_dispatcher_func_t, const void *, void *);
    static add_fn real;
    add_fn fn = __atomic_load_n(&real, __ATOMIC_ACQUIRE);
    if (!fn) {
        fn = (add_fn)dlsym(RTLD_NEXT, "wl_proxy_add_dispatcher");
        if (!fn) abort();
        __atomic_store_n(&real, fn, __ATOMIC_RELEASE);
    }
    int cls = (dispatcher && wlp_tracking()) ? disp_class(wl_proxy_get_class(proxy)) : 0;
    if (cls) {
        int placed = 0;
        pthread_mutex_lock(&g_wrap_lock);
        for (int i = 0; i < MAX_WRAPPED && !placed; i++)
            if (!g_disp[i].proxy) {
                g_disp[i] = (struct wrapped_disp){proxy, dispatcher, cls};
                placed = 1;
            }
        pthread_mutex_unlock(&g_wrap_lock);
        if (placed) dispatcher = disp_wrap;
        else note_wrap_failed();
    }
    return fn(proxy, dispatcher, dispatcher_data, data);
}

/* ---- exported hooks ---- */

/* GTK may register listeners before the shim has initialized; wlp_tracking() lets those
 * through to the wrapper and passes a disarmed, failed or re-entered call through untouched. */
int wl_proxy_add_listener(struct wl_proxy *proxy, void (**implementation)(void), void *data) {
    typedef int (*add_fn)(struct wl_proxy *, void (**)(void), void *);
    static add_fn real;
    add_fn fn = __atomic_load_n(&real, __ATOMIC_ACQUIRE);
    if (!fn) {
        fn = (add_fn)dlsym(RTLD_NEXT, "wl_proxy_add_listener");
        if (!fn) abort();
        __atomic_store_n(&real, fn, __ATOMIC_RELEASE);
    }
    if (wlp_tracking()) implementation = wlfocus_wrap(proxy, implementation);
    return fn(proxy, implementation, data);
}

void wl_proxy_destroy(struct wl_proxy *proxy) {
    typedef void (*destroy_fn)(struct wl_proxy *);
    static destroy_fn real;
    destroy_fn fn = __atomic_load_n(&real, __ATOMIC_ACQUIRE);
    if (!fn) {
        fn = (destroy_fn)dlsym(RTLD_NEXT, "wl_proxy_destroy");
        if (!fn) abort();
        __atomic_store_n(&real, fn, __ATOMIC_RELEASE);
    }
    /* Forget the proxy first so a reused address cannot meet a stale entry; free the table
     * only after libwayland is done with it. */
    wl_fn *copy = wlfocus_detach(proxy);
    disp_detach(proxy);
    fn(proxy);
    wlfocus_free(copy);
}
