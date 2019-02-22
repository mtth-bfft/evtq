#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include "render.h"
#include "mem.h"

int render_event_to_xml(HANDLE hEvent, PSTR *pszXML, PDWORD pdwXMLSize)
{
	int res = 0;
	DWORD dwBufRequired = 0;
	DWORD dwPropCount = 0;

	if (EvtRender(NULL, hEvent, EvtRenderEventXml, 0, NULL, &dwBufRequired, &dwPropCount) ||
		GetLastError() != ERROR_INSUFFICIENT_BUFFER)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to render event as XML, code %u\n"), res);
		goto cleanup;
	}

	PSTR szXML = safe_alloc(dwBufRequired);

	if (!EvtRender(NULL, hEvent, EvtRenderEventXml, dwBufRequired, szXML, &dwBufRequired, &dwPropCount))
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to render event as XML, code %u\n"), res);
		goto cleanup;
	}

	*pszXML = szXML;
	*pdwXMLSize = dwBufRequired;

cleanup:
	return res;
}

int render_event_to_text(HANDLE hEvent)
{
	int res = 0;
	HANDLE hCtxSystem = NULL;
	HANDLE hCtxUser = NULL;
	DWORD dwPropCount = 0;
	DWORD dwBufRequired = 0;
	DWORD dwBufSize = 0;
	PEVT_VARIANT pVars = NULL;

	hCtxSystem = EvtCreateRenderContext(0, NULL, EvtRenderContextSystem);
	if (hCtxSystem == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to create system event rendering context, code %u\n"), res);
		goto cleanup;
	}

	hCtxUser = EvtCreateRenderContext(0, NULL, EvtRenderContextUser);
	if (hCtxUser == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to create user event rendering context, code %u\n"), res);
		goto cleanup;
	}

	if (!EvtRender(hCtxSystem, hEvent, EvtRenderEventValues, dwBufSize, pVars, &dwBufRequired, &dwPropCount))
	{
		res = GetLastError();
		if (res == ERROR_INSUFFICIENT_BUFFER)
		{
			pVars = safe_realloc(pVars, dwBufRequired);
			dwBufSize = dwBufRequired;
			if (EvtRender(hCtxSystem, hEvent, EvtRenderEventValues, dwBufSize, pVars, &dwBufRequired, &dwPropCount))
				res = 0;
		}
		if (res != 0)
		{
			_ftprintf(stderr, TEXT("Error: unable to render event's system properties, code %u\n"), res);
			goto cleanup;
		}
	}
	if (!EvtRender(hCtxUser, hEvent, EvtRenderEventValues, dwBufSize, pVars, &dwBufRequired, &dwPropCount))
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to render event's user properties, code %u\n"), res);
		goto cleanup;
	}

cleanup:
	if (hCtxSystem != NULL)
		EvtClose(hCtxSystem);
	if (hCtxUser != NULL)
		EvtClose(hCtxUser);
	if (pVars != NULL)
		safe_free(pVars);
	return res;
}