# Measures WhiteHub resource usage: startup, idle, polling, UI open, UI closed.
# Usage: powershell -File scripts\measure.ps1 [-Exe path] [-IdleSeconds 360]
param(
  [string]$Exe = "$PSScriptRoot\..\src-tauri\target\release\whitehub.exe",
  [int]$IdleSeconds = 360
)
$ErrorActionPreference = 'Stop'
$log = "$env:LOCALAPPDATA\com.whitehub.desktop\logs\whitehub.log"

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process
  $ids = @($root); $added = $true
  while ($added) {
    $added = $false
    foreach ($p in $all) { if ($ids -contains $p.ParentProcessId -and -not ($ids -contains $p.ProcessId)) { $ids += $p.ProcessId; $added = $true } }
  }
  Get-Process -Id $ids -ErrorAction SilentlyContinue
}
function Show([string]$label, [int]$root) {
  $t = Get-Tree $root
  $main = $t | Where-Object Id -eq $root
  "{0,-28} procs={1} main WS={2,6:N1} MB  main private={3,5:N1} MB  tree private={4,6:N1} MB  tree CPU={5:N2}s" -f $label, $t.Count,
    ($main.WorkingSet64 / 1MB), ($main.PrivateMemorySize64 / 1MB), (($t | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB), (($t | Measure-Object CPU -Sum).Sum)
}

Get-Process whitehub -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 1
$before = if (Test-Path $log) { (Get-Content $log).Count } else { 0 }
$p = Start-Process $Exe -ArgumentList '--autostart' -PassThru
$start = $p.StartTime
Start-Sleep 5
$new = Get-Content $log | Select-Object -Skip $before
function Stamp($pattern) { $l = $new | Where-Object { $_ -match $pattern } | Select-Object -First 1; if ($l) { [datetime]::ParseExact($l.Substring(0, 23), 'yyyy-MM-dd HH:mm:ss.fff', $null) } }
$embedded = Stamp 'taskbar strip embedded'
$firstBattery = Stamp '\] battery \d+%'
"Startup: strip embedded after {0:N0} ms, first battery value after {1:N0} ms" -f ($embedded - $start).TotalMilliseconds, ($firstBattery - $start).TotalMilliseconds
Show 'after start (5 s)' $p.Id
Start-Sleep 15
Show 'idle, UI never opened (20 s)' $p.Id

$cpu0 = (Get-Process -Id $p.Id).CPU
Start-Sleep $IdleSeconds
$cpu1 = (Get-Process -Id $p.Id).CPU
$polls = (Get-Content $log | Select-Object -Skip $before | Where-Object { $_ -match 'poll took|\] battery|\] status' }).Count
"CPU over {0} s idle (includes scheduled polls): {1:N3} s = {2:N4} % of one core" -f $IdleSeconds, ($cpu1 - $cpu0), (($cpu1 - $cpu0) / $IdleSeconds * 100)
Show "idle ($IdleSeconds s later)" $p.Id

Start-Process $Exe | Out-Null   # second instance -> opens the window
Start-Sleep 6
Show 'UI open' $p.Id
$w = Get-Process -Id $p.Id
Add-Type -Namespace M -Name U -MemberDefinition '[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string n); [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);'
[void][M.U]::PostMessageW([M.U]::FindWindowW('Tauri Window', 'WhiteHub'), 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)  # WM_CLOSE to the popup
Start-Sleep 15
Show 'UI closed (15 s later)' $p.Id
