/*
 * wlfocus_slots.h - pure listener-slot helpers shared by wlfocus.c and wlfocus_selftest.c.
 *
 * BENCH TOOLING ONLY. Nothing here touches shim state, so the self-test can include it
 * without linking the shim. Everything is static: no symbol leaks into an LD_PRELOADed app.
 */
#ifndef WLFOCUS_SLOTS_H
#define WLFOCUS_SLOTS_H

#include <ctype.h>
#include <stdint.h>
#include <wayland-client.h>

/* Event opcodes of the slots wlfocus wraps (the self-test checks them against libwayland). */
enum { KB_ENTER = 1, KB_LEAVE = 2, PTR_ENTER = 0, PTR_LEAVE = 1, TL_CONFIGURE = 0 };

/* Version an event was introduced in: the leading decimal digits of its signature, else 1. */
static inline uint32_t event_since(const struct wl_message *event) {
    const char *sig = event->signature;
    if (!sig || !isdigit((unsigned char)*sig)) return 1;
    uint32_t since = 0;
    for (; isdigit((unsigned char)*sig); sig++) since = since * 10 + (uint32_t)(*sig - '0');
    return since;
}

/* A client built against older protocol headers hands over a table with fewer slots than the
 * interface has events; only slots for events its bound version delivers are safe to read.
 * Version 0 provides nothing: the caller fails closed instead of reading any slot. */
static inline int slot_provided(const struct wl_interface *iface, unsigned idx, uint32_t version) {
    return event_since(&iface->events[idx]) <= version;
}

#endif
