#pragma once
#include <Windows.h>
#include <winevt.h>

typedef enum {
   INPUT_DEFAULT = 0,
   INPUT_EVTX,
   INPUT_EVT,
   INPUT_LOCALHOST,
   INPUT_REMOTEHOST,
} input_t;

typedef enum {
   OUTPUT_DEFAULT = 0,
   OUTPUT_TSV,
   OUTPUT_XML,
   OUTPUT_JSON,
} output_t;

extern BOOL bAppend;
extern BOOL bEver;
extern BOOL bGzip;
extern long eventCount;
extern int verbosity;
extern input_t input;
extern output_t output;

int render_event_callback(EVT_HANDLE hEvent);