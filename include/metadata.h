#pragma once
#include <Windows.h>

int init_fieldnames_from_system();
int init_fieldnames_from_export(PCTSTR swzFilePath);
int export_fieldnames_to_file(PCTSTR swzFilePath);
int get_event_field_name(PCWSTR swzPublisherName, UINT32 uEventID, UINT32 uEventVersion, DWORD dwFieldNumber, PCSTR *pszFieldName);
