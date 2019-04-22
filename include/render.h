#pragma once
#include <Windows.h>

void strip_non_printable_chars(PSTR szValue);
PSTR render_field(PEVT_VARIANT pVar);

/**
 * Function to be called before any rendering output is done,
 * to initialize synchronisation primitives between
 * begin_render_output() in different threads.
 */
int init_render_output();

/**
 * Function to be called by the output module right before it starts
 * emitting any output. Should be called as late as possile, so as to not
 * stall other rendering threads.
 */
int begin_render_output();

/**
 * Function to be called by the output module right after it ends
 * emitting any output. Should be called as early as possile, so as to not
 * stall other rendering threads.
 */
int end_render_output();
