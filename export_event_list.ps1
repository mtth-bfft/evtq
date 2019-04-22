function Export-EventList {
    $provs = Get-WinEvent -ListProvider '*'
    $provCount = 0;
    $knownFieldNames = @{}
    $provs | % {
        $provname = $_.Name
        $provCount += 1;
        $_.Events | % {
            $id = $_.Id;
            $version = $_.Version;
            [xml]$fields_xml = $_.Template
            $fieldNames = @()
            $broken = $false;
            $fields_xml.template.ChildNodes | % {
                if ($_.name.length -lt 1) {
                    $broken = $true;
                }
            }
            if (!$broken) {
                $fields_xml.template.ChildNodes | % {
                    $fieldNames += $_.name
                }
            }
            $knownFieldNames.Add("${provname}-${id}-${version}", $fieldNames);
        }
        Write-Progress -Activity "Exporting providers and events" -status "Found provider $provname" -percentComplete ($provCount/$provs.Length*100)
    }
    return $knownFieldNames
}

if ($MyInvocation.InvocationName -ne '.') {
    $OutPath = '.\export_event_list.json'
    Export-EventList | ConvertTo-Json | Out-File -Encoding utf8 -FilePath $OutPath
    # Remove BOM (libjansson does not detect it, and faults when given a FILE* configured with the right encoding)
    $utf8nobom = New-Object System.Text.UTF8Encoding($False)
    [System.IO.File]::WriteAllLines($OutPath, (Get-Content $OutPath), $utf8nobom)
    Write-Host -ForegroundColor Green "[Done]"
}