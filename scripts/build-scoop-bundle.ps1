param(
    [string]$OutDir = "$PSScriptRoot\..\src-tauri\resources",
    [string]$Work = "$env:TEMP\scoop-bundle-build"
)

$ErrorActionPreference = 'Stop'
$bsdtar = "$env:SystemRoot\System32\tar.exe"

if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
$root = "$Work\root"
New-Item -ItemType Directory "$root\apps" -Force | Out-Null
New-Item -ItemType Directory "$root\buckets" -Force | Out-Null
New-Item -ItemType Directory $OutDir -Force | Out-Null

Write-Host 'Cloning scoop (shallow)...'
git clone --depth 1 https://gitee.com/scoop-installer/scoop.git "$root\apps\scoop\current"
Write-Host 'Cloning Main bucket (shallow)...'
git clone --depth 1 https://gitee.com/scoop-installer/Main.git "$root\buckets\main"

$scoopCommit = (git -C "$root\apps\scoop\current" rev-parse HEAD).Trim()
$mainCommit = (git -C "$root\buckets\main" rev-parse HEAD).Trim()

$meta = [ordered]@{
    built = (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
    scoop = [ordered]@{ repo = 'https://gitee.com/scoop-installer/scoop'; commit = $scoopCommit }
    main  = [ordered]@{ repo = 'https://gitee.com/scoop-installer/Main';  commit = $mainCommit }
}
$metaJson = ($meta | ConvertTo-Json -Depth 5)
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText("$root\bundle.json", $metaJson, $utf8NoBom)
[System.IO.File]::WriteAllText("$OutDir\scoop-bundle.json", $metaJson, $utf8NoBom)

$zip = "$OutDir\scoop-bundle.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Write-Host 'Compressing (bsdtar)...'
& $bsdtar -a -c -f $zip -C $root apps buckets bundle.json
if ($LASTEXITCODE -ne 0) { throw "bsdtar failed: $LASTEXITCODE" }

$size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "scoop=$scoopCommit main=$mainCommit size=${size}MB"
Write-Host "zip: $zip"
