#pragma once
#include <Windows.h>
#include <winevt.h>

int render_event_tsv(FILE *out, EVT_HANDLE hEvent);
