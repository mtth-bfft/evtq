#include <Windows.h>
#include <winevt.h>
#include <tchar.h>
#include <stdio.h>
#include "outputs\xml.h"
#include "render.h"
#include "mem.h"

int render_event_xml(FILE *out, EVT_HANDLE hEvent)
{
   int res = 0;
   PWSTR swzXml = NULL;
   DWORD dwBufferSize = 0;
   DWORD dwPropertyCount = 0;

   if (EvtRender(NULL, hEvent, EvtRenderEventXml, 0, NULL, &dwBufferSize, &dwPropertyCount) ||
      GetLastError() != ERROR_INSUFFICIENT_BUFFER)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Error: unable to render event as xml, code %u\n"), res);
      goto cleanup;
   }
   swzXml = (PWSTR)safe_alloc(dwBufferSize);
   if (!EvtRender(NULL, hEvent, EvtRenderEventXml, dwBufferSize, swzXml, &dwBufferSize, &dwPropertyCount))
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Error: unable to render event as xml (2), code %u\n"), res);
      goto cleanup;
   }

   res = begin_render_output();
   if (res != 0)
      goto cleanup;

   fwprintf(out, L"%s\n", swzXml);

   res = end_render_output();

cleanup:
   if (swzXml != NULL)
      safe_free(swzXml);
   return res;
}
