#pragma once
#include <Windows.h>
#include <winevt.h>
#include <stdio.h>

int render_event_json(FILE *out, EVT_HANDLE hEvent);
