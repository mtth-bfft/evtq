#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include "inputs/live.h"
#include "main.h"
#include "mem.h"

static LONG64 dwEventCount = 0;

DWORD WINAPI callback_source_live(EVT_SUBSCRIBE_NOTIFY_ACTION Action, PVOID _pContext, EVT_HANDLE hEvent)
{
   (void)(_pContext);

	if (Action != EvtSubscribeActionDeliver)
	{
		printf(" [!] Unable to read event from source: error code %zu \n", (SIZE_T)hEvent);
		return 0;
	}

	// Bump the event count, so the main thread doesn't kill
	// us thinking no more events are coming in (there's no proper mechanism
	// in push subscription to detect the end of stream)
	_InlineInterlockedAdd64(&dwEventCount, 1);

   render_event_callback(hEvent);
	return 0;
}

int open_source_live(PWSTR swzHostname, PWSTR swzDomain, PWSTR swzUser, PWSTR swzPassword, BOOL bFollow)
{
	int res = 0;
	EVT_HANDLE hSession = NULL;
	EVT_RPC_LOGIN rpcLogin = { 0 };
	HANDLE hChannelEnum = NULL;
	DWORD dwChannelCount = 0;
	PHANDLE phFeed = NULL;
	WCHAR swzChannel[MAX_PATH] = { 0 };
	DWORD dwChannelLen = MAX_PATH;
	DWORD dwBytesRequired = 0;

	if (swzHostname != NULL)
	{
		rpcLogin.Server = swzHostname;
		rpcLogin.Domain = swzDomain;
		rpcLogin.User = swzUser;
		rpcLogin.Password = swzPassword;
		rpcLogin.Flags = EvtRpcLoginAuthNegotiate;

		hSession = EvtOpenSession(EvtRpcLogin, &rpcLogin, 0, 0);
		if (swzPassword != NULL)
		{
			ZeroMemory(swzPassword, wcslen(swzPassword) * sizeof(WCHAR));
		}
		if (hSession == NULL)
		{
			res = GetLastError();
			_ftprintf(stderr, TEXT("Error: unable to connect to remote host, code %u\n"), res);
			goto cleanup;
		}
	}

	hChannelEnum = EvtOpenChannelEnum(hSession, 0);
	if (hChannelEnum == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to enumerate channels, code %u\n"), res);
		goto cleanup;
	}

	while (EvtNextChannelPath(hChannelEnum, dwChannelLen, swzChannel, &dwBytesRequired))
	{
		HANDLE hFeed = EvtSubscribe(hSession, NULL, swzChannel, NULL, NULL, NULL, &callback_source_live,
         (bFollow ? EvtSubscribeToFutureEvents : EvtSubscribeStartAtOldestRecord));
		if (hFeed == NULL)
		{
			if (GetLastError() == ERROR_EVT_SUBSCRIPTION_TO_DIRECT_CHANNEL)
				continue; // skip silently channels that can't be subscribed to
         if (verbosity > 0)
			   _ftprintf(stderr, TEXT("Error: unable to subscribe to events on '%ws', code %u\n"), swzChannel, GetLastError());
		}
		dwChannelCount++;
		phFeed = (PHANDLE)safe_realloc(phFeed, sizeof(HANDLE) * dwChannelCount);
		phFeed[dwChannelCount - 1] = hFeed;
	}
	if (GetLastError() != ERROR_NO_MORE_ITEMS)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error while enumerating channels, code %u\n"), res);
		goto cleanup;
	}

   if (verbosity > 0)
	   printf("Waiting for the end...\n");

	if (bFollow)
	{
		while (1)
		{
			Sleep(1000);
			// TODO: handle Ctrl-C
		}
	}
	else
	{
		while (1) {
			LONG64 dwPrevEventCount = dwEventCount;
			Sleep(1000);
			if (dwPrevEventCount == dwEventCount)
			{
				break;
			}
		}
	}

	printf("Done.\n");

cleanup:
	if (hSession != NULL)
		EvtClose(hSession);
	return res;
}