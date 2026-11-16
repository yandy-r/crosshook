/*
 * wlpresent.h - declarations shared by wlpresent.c and wlfocus.c.
 *
 * BENCH TOOLING ONLY. Every symbol here is hidden: the shim is LD_PRELOADed into the app,
 * so a global "rec" or "now_ns" must never interpose on (or be interposed by) the app's own.
 */
#ifndef WLPRESENT_H
#define WLPRESENT_H

#include <pthread.h>
#include <stdint.h>
#include <wayland-client.h>

#define UNUSED __attribute__((unused))
#define ULL(x) ((unsigned long long)(x))
#define WLP_HIDDEN __attribute__((visibility("hidden")))

typedef void (*wl_fn)(void);

struct surf {
    uint32_t wl_id, xdg_id; /* wl_id == 0: free slot */
    uint32_t toplevel_id;   /* wl id of the xdg_toplevel created from xdg_id, 0 if none */
    int toplevel, has_buffer;
    uint64_t toplevel_ns;
    /* Latest xdg_toplevel.configure seen before the log existed; flushed after the header. */
    int cfg_pending, cfg_w, cfg_h, cfg_activated;
    uint64_t cfg_ns;
};

/* wlpresent.c */
WLP_HIDDEN extern pthread_mutex_t g_tab_lock;
WLP_HIDDEN extern int g_state; /* 0 not initialized, 1 ready, -1 pass-through */
WLP_HIDDEN uint64_t now_ns(void);
WLP_HIDDEN void rec(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
WLP_HIDDEN int wlp_tracking(void);
/* Callers hold g_tab_lock. */
WLP_HIDDEN struct surf *surf_find_toplevel(uint32_t toplevel_id);

/* wlfocus.c */
/* Returns the table to hand to the real wl_proxy_add_listener: a wrapped copy for
 * wl_keyboard/wl_pointer/xdg_toplevel proxies, otherwise impl itself. */
WLP_HIDDEN wl_fn *wlfocus_wrap(struct wl_proxy *proxy, wl_fn *impl);
/* Forgets a proxy that is about to be destroyed and returns its copied table (or NULL).
 * Call before the real destroy; pass the result to wlfocus_free after it returns. */
WLP_HIDDEN wl_fn *wlfocus_detach(struct wl_proxy *proxy);
WLP_HIDDEN void wlfocus_free(wl_fn *copy);
/* Writes a wrap_failed queued before the log existed; the caller holds g_tab_lock. */
WLP_HIDDEN void wlfocus_flush_pending(void);
WLP_HIDDEN void rec_geometry(uint64_t ns, uint32_t surface_id, int w, int h, int activated);

#endif
