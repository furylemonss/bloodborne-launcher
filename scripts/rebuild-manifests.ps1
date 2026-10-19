# expected-*.tsv yeniden üretir. Golden ağaçlar yoksa çalışmaz.
# Kullanım (oyun çıkarılmışken):
#   powershell -ExecutionPolicy Bypass -File scripts\rebuild-manifests.ps1 -Root 'D:\İndirilenler\Bloodborne\Installer'
param(
  [Parameter(Mandatory = $true)]
  [string]$Root
)

$ErrorActionPreference = 'Stop'
$base = Join-Path $Root 'downloads\CUSA03173'
$patch = Join-Path $Root 'downloads\CUSA03173-patch'
$outDir = Join-Path $Root 'src-tauri'

function Write-Manifest([string]$src, [string]$dst) {
  if (-not (Test-Path -LiteralPath $src)) {
    throw "Golden agac yok: $src"
  }
  $lines = Get-ChildItem -LiteralPath $src -Recurse -File -Force |
    ForEach-Object {
      $rel = $_.FullName.Substring($src.Length).TrimStart('\', '/')
      "{0}`t{1}" -f $rel.Replace('\', '/'), $_.Length
    }
  $lines | Set-Content -LiteralPath $dst -Encoding utf8
  Write-Host ("wrote {0}  lines={1}" -f $dst, $lines.Count)
}

Write-Manifest $base (Join-Path $outDir 'expected-base.tsv')
Write-Manifest $patch (Join-Path $outDir 'expected-patch.tsv')

Write-Host '--- base head ---'
Get-Content -LiteralPath (Join-Path $outDir 'expected-base.tsv') -TotalCount 5
Write-Host '--- patch head ---'
Get-Content -LiteralPath (Join-Path $outDir 'expected-patch.tsv') -TotalCount 5
