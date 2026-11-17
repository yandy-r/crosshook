/*
 * wlfocus_selftest.c - checks the pure listener-slot helpers against libwayland's own
 * wl_pointer and wl_keyboard interface tables. BENCH TOOLING ONLY; built and run by
 * `bench_shim.py --check`, never loaded into an app. Exits non-zero on any failure.
 */
#include <stdio.h>
#include <string.h>
#include "wlfocus_slots.h"

static int g_failures;

#define CHECK(cond)                                                              \
    do {                                                                         \
        if (!(cond)) {                                                           \
            fprintf(stderr, "wlfocus_selftest: line %d: %s\n", __LINE__, #cond); \
            g_failures++;                                                        \
        }                                                                        \
    } while (0)

static const struct wl_message *find_event(const struct wl_interface *iface, const char *name) {
    for (int i = 0; i < iface->event_count; i++)
        if (strcmp(iface->events[i].name, name) == 0) return &iface->events[i];
    return NULL;
}

static int event_index(const struct wl_interface *iface, const char *name) {
    const struct wl_message *event = find_event(iface, name);
    return event ? (int)(event - iface->events) : -1;
}

static void check_since(const struct wl_interface *iface, const char *name, uint32_t since) {
    const struct wl_message *event = find_event(iface, name);
    if (!event) {
        fprintf(stderr, "wlfocus_selftest: %s has no event %s\n", iface->name, name);
        g_failures++;
        return;
    }
    uint32_t got = event_since(event);
    if (got != since) {
        fprintf(stderr, "wlfocus_selftest: %s.%s since %u, expected %u\n", iface->name, name, got, since);
        g_failures++;
    }
}

static void test_signature_parsing(void) {
    struct wl_message m = {"synthetic", NULL, NULL};
    CHECK(event_since(&m) == 1); /* no signature */
    m.signature = "";
    CHECK(event_since(&m) == 1);
    m.signature = "uoo";
    CHECK(event_since(&m) == 1); /* no leading version */
    m.signature = "?o";
    CHECK(event_since(&m) == 1);
    m.signature = "5";
    CHECK(event_since(&m) == 5);
    m.signature = "8uu";
    CHECK(event_since(&m) == 8);
    m.signature = "10u";
    CHECK(event_since(&m) == 10); /* more than one digit */
    m.signature = "12?o";
    CHECK(event_since(&m) == 12);
}

static void test_event_versions(void) {
    check_since(&wl_pointer_interface, "enter", 1);
    check_since(&wl_pointer_interface, "leave", 1);
    check_since(&wl_pointer_interface, "frame", 5);
    check_since(&wl_pointer_interface, "axis_source", 5);
    check_since(&wl_pointer_interface, "axis_value120", 8);
    check_since(&wl_pointer_interface, "axis_relative_direction", 9);
    check_since(&wl_keyboard_interface, "keymap", 1);
    check_since(&wl_keyboard_interface, "enter", 1);
    check_since(&wl_keyboard_interface, "leave", 1);
    check_since(&wl_keyboard_interface, "repeat_info", 4);
}

/* The wrapped opcodes must name the events they claim to. */
static void test_wrapped_opcodes(void) {
    CHECK(event_index(&wl_keyboard_interface, "enter") == KB_ENTER);
    CHECK(event_index(&wl_keyboard_interface, "leave") == KB_LEAVE);
    CHECK(event_index(&wl_pointer_interface, "enter") == PTR_ENTER);
    CHECK(event_index(&wl_pointer_interface, "leave") == PTR_LEAVE);
}

static void test_slot_provided(void) {
    const struct wl_interface *ptr = &wl_pointer_interface, *kb = &wl_keyboard_interface;
    int frame = event_index(ptr, "frame"), v120 = event_index(ptr, "axis_value120");
    int repeat = event_index(kb, "repeat_info");
    CHECK(frame >= 0 && v120 >= 0 && repeat >= 0);

    CHECK(slot_provided(ptr, PTR_ENTER, 1));
    CHECK(slot_provided(ptr, PTR_LEAVE, 1));
    CHECK(!slot_provided(ptr, (unsigned)frame, 4));
    CHECK(slot_provided(ptr, (unsigned)frame, 5));
    CHECK(!slot_provided(ptr, (unsigned)v120, 7));
    CHECK(slot_provided(ptr, (unsigned)v120, 8));
    CHECK(slot_provided(ptr, (unsigned)v120, 9));
    CHECK(slot_provided(kb, KB_ENTER, 1));
    CHECK(!slot_provided(kb, (unsigned)repeat, 3));
    CHECK(slot_provided(kb, (unsigned)repeat, 4));

    /* Version 0 provides no slot at all: the shim must not read any of a table it cannot size. */
    for (int i = 0; i < ptr->event_count; i++) CHECK(!slot_provided(ptr, (unsigned)i, 0));
    for (int i = 0; i < kb->event_count; i++) CHECK(!slot_provided(kb, (unsigned)i, 0));

    /* At the interface's own version every event is delivered. */
    for (int i = 0; i < ptr->event_count; i++) CHECK(slot_provided(ptr, (unsigned)i, (uint32_t)ptr->version));
    for (int i = 0; i < kb->event_count; i++) CHECK(slot_provided(kb, (unsigned)i, (uint32_t)kb->version));
}

int main(void) {
    test_signature_parsing();
    test_event_versions();
    test_wrapped_opcodes();
    test_slot_provided();
    if (g_failures) {
        fprintf(stderr, "wlfocus_selftest: %d check(s) failed\n", g_failures);
        return 1;
    }
    puts("wlfocus_selftest: ok");
    return 0;
}
