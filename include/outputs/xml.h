#pragma once
#include <Windows.h>
#include <winevt.h>
#include <stdio.h>

int render_event_xml(FILE *out, EVT_HANDLE hEvent);
