#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include "sources/live.h"
#include "mem.h"

static LONG64 dwEventCount = 0;

DWORD WINAPI callback_source_live(EVT_SUBSCRIBE_NOTIFY_ACTION Action, PVOID pContext, EVT_HANDLE hEvent)
{
	if (Action != EvtSubscribeActionDeliver)
	{
		printf(" [!] Received error %zu\n", (SIZE_T)hEvent);
		return 0;
	}

	// Bump the event count, so the main thread doesn't kill
	// us thinking no more events are coming in (there's no proper mechanism
	// in push subscription to detect the end of stream)
	_InlineInterlockedAdd64(&dwEventCount, 1);

	(void)(hEvent);
	(void)(pContext);
	printf(".");
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
		HANDLE hFeed = EvtSubscribe(hSession, NULL, swzChannel, NULL, NULL, NULL, &callback_source_live, EvtSubscribeStartAtOldestRecord | EvtSubscribeStrict);
		if (hFeed == NULL)
		{
			if (GetLastError() == ERROR_EVT_SUBSCRIPTION_TO_DIRECT_CHANNEL)
				continue; // skip silently channels that can't be subscribed to
			_ftprintf(stderr, TEXT("Error: unable to subscribe to events, code %u\n"), GetLastError());
		}
		dwChannelCount++;
		phFeed = safe_realloc(phFeed, sizeof(HANDLE) * dwChannelCount);
		phFeed[dwChannelCount - 1] = hFeed;
	}
	if (GetLastError() != ERROR_NO_MORE_ITEMS)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error while enumerating channels, code %u\n"), res);
		goto cleanup;
	}

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