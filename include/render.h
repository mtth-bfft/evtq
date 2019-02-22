#pragma once

#include <Windows.h>

int render_event_to_xml(HANDLE hEvent, PSTR *pszXML, PDWORD pdwXMLSize);
int render_event_to_text(HANDLE hEvent);
