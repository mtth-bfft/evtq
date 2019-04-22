#include <Windows.h>
#include <winevt.h>
#include <tchar.h>
#include <stdio.h>
#include <map>
#include <vector>
#include <jansson.h>
#include "mem.h"
#include "metadata.h"

// This is the only C++ part of the codebase. A hashmap from strings
// (<providername>-<eventid>-<version>) to vectors of strings (field names).
// No need to wrap accesses to this map through a mutex, as it is initialized
// at start time and then only used read-only.
static std::map<std::string, std::vector<std::string>> knownFieldNames;

static BOOL bInitFromSystemDone = FALSE;

static int init_fieldnames_from_system_event(PCWSTR swzPublisherName, EVT_HANDLE hEvent);

int export_fieldnames_to_file(PCTSTR swzFilePath)
{
    int res = 0;
    std::map<std::string, std::vector<std::string>>::iterator it;
    json_t *jMap = json_object();
    FILE *fExport = NULL;

    res = _tfopen_s(&fExport, swzFilePath, TEXT("w"));
    if (res != 0)
    {
        _ftprintf(stderr, TEXT("Error: unable to open event field name backup file '%s', code %u\n"), swzFilePath, res);
        goto cleanup;
    }

    _tprintf(TEXT(" [.] Exporting publishers and field names to '%s' ...\n"), swzFilePath);

    for (it = knownFieldNames.begin(); it != knownFieldNames.end(); it++)
    {
        std::vector<std::string>::iterator it2;
        json_t *jFieldNames = json_array();

        for (it2 = it->second.begin(); it2 != it->second.end(); it2++)
        {
            json_array_append_new(jFieldNames, json_string((*it2).c_str()));
        }
        json_object_set_new(jMap, it->first.c_str(), jFieldNames);
    }

    _tprintf(TEXT(" [.] Writing to file...\n"));
    res = json_dumpf(jMap, fExport, JSON_INDENT(2) | JSON_COMPACT);
    if (res != 0)
    {
        _ftprintf(stderr, TEXT("Error: unable to serialize event field names into backup file '%s', code %u\n"), swzFilePath, res);
        goto cleanup;
    }

    _tprintf(TEXT(" [.] Done exporting.\n"));

cleanup:
    if (jMap != NULL)
        json_decref(jMap);
    if (fExport != NULL)
        fclose(fExport);
    return res;
}

int init_fieldnames_from_export(PCTSTR swzFilePath)
{
    int res = 0;
    FILE *fBackup = NULL;
    json_t *jMap = NULL;
    json_error_t jError;
    PCSTR szHashKey = NULL;
    json_t *jFieldNames = NULL;

    res = _tfopen_s(&fBackup, swzFilePath, TEXT("r"));
    if (res != 0)
    {
        _ftprintf(stderr, TEXT("Error: unable to open event field name list '%s', code %u\n"), swzFilePath, res);
        goto cleanup;
    }

    jMap = json_loadf(fBackup, 0, &jError);
    if (!jMap)
    {
        _ftprintf(stderr, TEXT("Error: unable to parse event field name list '%s' as JSON (%hs at line %d)\n"),
            swzFilePath, jError.text, jError.line);
        goto cleanup;
    }

    _tprintf(TEXT(" [.] Importing publishers and field names from '%s' ...\n"), swzFilePath);
    json_object_foreach(jMap, szHashKey, jFieldNames)
    {
        SIZE_T dwIndex = 0;
        json_t *jFieldName = NULL;
        if (knownFieldNames.count(szHashKey) == 0)
        {
            std::vector<std::string> fieldNames;
            knownFieldNames[szHashKey] = fieldNames;
        }
        if (knownFieldNames[szHashKey].size() < json_array_size(jFieldNames))
        {
            knownFieldNames[szHashKey].resize(json_array_size(jFieldNames), "");
        }
        json_array_foreach(jFieldNames, dwIndex, jFieldName)
        {
            knownFieldNames[szHashKey][dwIndex] = json_string_value(jFieldName);
        }
    }
    _tprintf(TEXT(" [.] Done importing.\n"));

cleanup:
    if (jMap != NULL)
        json_decref(jMap);
    if (fBackup != NULL)
        fclose(fBackup);
    return res;
}

int init_fieldnames_from_system()
{
   int res = 0;
   EVT_HANDLE hPublishers = NULL;
   DWORD dwBufferSize = 0;
   DWORD dwBufferReq = 0;
   PWSTR swzPublisherName = NULL;
   EVT_HANDLE hPublisherEvents = NULL;
   EVT_HANDLE hPublisher = NULL;

   if (bInitFromSystemDone)
       goto cleanup;
   bInitFromSystemDone = TRUE;

   hPublishers = EvtOpenPublisherEnum(NULL, 0);
   if (hPublishers == NULL)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Warning: unable to enumerate publishers, code %u\n"), res);
      goto cleanup;
   }

   while (1)
   {
      if (!EvtNextPublisherId(hPublishers, dwBufferSize, swzPublisherName, &dwBufferReq))
      {
         if (GetLastError() == ERROR_NO_MORE_ITEMS)
         {
            break;
         }
         else if (GetLastError() == ERROR_INSUFFICIENT_BUFFER)
         {
            dwBufferSize = dwBufferReq + 1;
            swzPublisherName = (PTSTR)safe_realloc(swzPublisherName, (dwBufferReq + 1) * sizeof(TCHAR));
            continue;
         }
         else
         {
            _ftprintf(stderr, TEXT("Warning: unable to get next publisher name, code %u\n"), GetLastError());
            break;
         }
      }
      hPublisher = EvtOpenPublisherMetadata(NULL, swzPublisherName, NULL, 0, 0);
      if (hPublisher == NULL)
      {
         _ftprintf(stderr, TEXT("Warning: unable to query provider '%ws' metadata, code %u\n"), swzPublisherName, GetLastError());
         continue;
      }
      hPublisherEvents = EvtOpenEventMetadataEnum(hPublisher, 0);
      if (hPublisherEvents == NULL)
      {
         _ftprintf(stderr, TEXT("Warning: unable to query provider '%ws' events, code %u\n"), swzPublisherName, GetLastError());
         EvtClose(hPublisher);
         continue;
      }
      while (1)
      {
         EVT_HANDLE hEvent = EvtNextEventMetadata(hPublisherEvents, 0);
         if (hEvent == NULL)
         {
            if (GetLastError() != ERROR_NO_MORE_ITEMS)
               _ftprintf(stderr, TEXT("Warning: unable to query provider '%ws' event, code %u\n"), swzPublisherName, res);
            break;
         }
         init_fieldnames_from_system_event(swzPublisherName, hEvent);
         EvtClose(hEvent);
      }
      EvtClose(hPublisher);
   }

   printf(" [.] Done initializing\n");

cleanup:
   if (hPublishers != NULL)
      EvtClose(hPublishers);
   return res;
}

static int init_fieldnames_from_system_event(PCWSTR swzPublisherName, EVT_HANDLE hEvent)
{
   int res = 0;
   DWORD dwEventIDBufLen = sizeof(EVT_VARIANT) + sizeof(UINT32);
   BYTE bufEventID[sizeof(EVT_VARIANT) + sizeof(UINT32)] = { 0 };
   DWORD dwEventVersionBufLen = sizeof(EVT_VARIANT) + sizeof(UINT32);
   BYTE bufEventVersion[sizeof(EVT_VARIANT) + sizeof(UINT32)] = { 0 };
   DWORD dwEventTemplateBufLen = 0;
   PEVT_VARIANT pEventTemplate = NULL;
   std::vector<std::string> fieldNames;

   if (!EvtGetEventMetadataProperty(hEvent, EventMetadataEventID, 0, dwEventIDBufLen, (PEVT_VARIANT)&bufEventID, &dwEventIDBufLen))
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Warning: unable to query event ID from publisher '%s', code %u\n"), swzPublisherName, res);
      goto cleanup;
   }
   if (!EvtGetEventMetadataProperty(hEvent, EventMetadataEventVersion, 0, dwEventVersionBufLen, (PEVT_VARIANT)&bufEventVersion, &dwEventVersionBufLen))
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Warning: unable to query event version from publisher '%s', code %u\n"), swzPublisherName, res);
      goto cleanup;
   }
   if (!EvtGetEventMetadataProperty(hEvent, EventMetadataEventTemplate, 0, 0, NULL, &dwEventTemplateBufLen) && GetLastError() != ERROR_INSUFFICIENT_BUFFER)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Warning: unable to query event template from publisher '%s', code %u\n"), swzPublisherName, res);
      goto cleanup;
   }
   pEventTemplate = (PEVT_VARIANT)safe_alloc(dwEventTemplateBufLen);
   if (!EvtGetEventMetadataProperty(hEvent, EventMetadataEventTemplate, 0, dwEventTemplateBufLen, pEventTemplate, &dwEventTemplateBufLen))
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Warning: unable to query event template from publisher '%s', code %u\n"), swzPublisherName, res);
      goto cleanup;
   }

   for (SIZE_T i = 0; i < wcslen(pEventTemplate->StringVal); i++)
   {
      PCWCHAR swzFieldNameEnd = NULL;
      PCWCHAR swzFieldNameStart = &(pEventTemplate->StringVal[i]);
      SIZE_T dwFieldNameLen = 0;
      if (_wcsnicmp(swzFieldNameStart, L"<data name=\"", wcslen(L"<data name=\"")) == 0)
      {
         swzFieldNameStart += wcslen(L"<data name=\"");
         swzFieldNameEnd = wcsstr(swzFieldNameStart, L"\"");
         if (swzFieldNameEnd == NULL)
         {
            _ftprintf(stderr, TEXT("Warning: unable to parse template from publisher '%s' event %u version %u\n"),
               swzPublisherName, ((PEVT_VARIANT)&bufEventID)->UInt32Val, ((PEVT_VARIANT)&bufEventVersion)->UInt32Val);
            goto cleanup;
         }
         dwFieldNameLen = swzFieldNameEnd - swzFieldNameStart;
         if (dwFieldNameLen > 0)
         {
            std::string fieldName(safe_strconv(swzFieldNameStart, dwFieldNameLen));
            fieldNames.push_back(fieldName);
         }
      }
      swzFieldNameStart++;
   }
   if (fieldNames.size() > 0)
   {
      CHAR szHashKey[255] = { 0 };
      sprintf_s(szHashKey, 255, "%ws-%u-%u", swzPublisherName, ((PEVT_VARIANT)&bufEventID)->UInt32Val, ((PEVT_VARIANT)&bufEventVersion)->UInt32Val);
      knownFieldNames[szHashKey] = fieldNames;
   }

cleanup:
   return res;
}

int get_event_field_name(PCWSTR swzPublisherName, UINT32 uEventID, UINT32 uEventVersion, DWORD dwFieldNumber, PCSTR *pszFieldName)
{
   CHAR szHashKey[255] = { 0 };
   sprintf_s(szHashKey, 255, "%ws-%u-%u", swzPublisherName, uEventID, uEventVersion);
   std::map<std::string, std::vector<std::string>>::iterator it = knownFieldNames.find(szHashKey);
   if (it != knownFieldNames.end())
   {
      if (dwFieldNumber < it->second.size())
      {
         *pszFieldName = it->second[dwFieldNumber].c_str();
         return 0;
      }
      return 1;
   }
   return 2;
}
