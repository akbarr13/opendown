param([string]$Action = "show")
# Simulates the shell's tray callback (msg 6002) for the app's "tray_icon_app" window.
# Ensures the hidden-icons flyout is open first (Shell_NotifyIconGetRect needs a position).
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class Sim {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string win);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$anyCond = [System.Windows.Automation.Condition]::TrueCondition
$Root = [System.Windows.Automation.AutomationElement]::RootElement
function Get-FlyoutH { return [Sim]::FindWindow("NotifyIconOverflowWindow", $null) }
function Ensure-Flyout {
  if ((Get-FlyoutH) -ne [IntPtr]::Zero) { Write-Host "flyout already open"; return $true }
  $c = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ClassNameProperty, "Shell_TrayWnd")
  $tb = $Root.FindFirst([System.Windows.Automation.TreeScope]::Children, $c)
  if (-not $tb) { return $false }
  $all = $tb.FindAll([System.Windows.Automation.TreeScope]::Descendants, $anyCond)
  $tbr = $tb.Current.BoundingRectangle
  foreach ($e in $all) {
    $r = $e.Current.BoundingRectangle
    if (-not $e.Current.Name -and $r.Width -ge 14 -and $r.Width -le 34 -and $r.Height -ge 16 -and $r.X -gt ($tbr.X + $tbr.Width - 300)) {
      Write-Host "opening flyout (chevron)"
      [void][Sim]::SetCursorPos([int]($r.X + $r.Width/2), [int]($r.Y + $r.Height/2))
      Start-Sleep -Milliseconds 250
      [Sim]::mouse_event(0x2,0,0,0,[IntPtr]::Zero); [Sim]::mouse_event(0x4,0,0,0,[IntPtr]::Zero)
      for ($i = 0; $i -lt 10; $i++) {
        Start-Sleep -Milliseconds 400
        if ((Get-FlyoutH) -ne [IntPtr]::Zero) { Write-Host "flyout opened"; return $true }
      }
      return $false
    }
  }
  return $false
}
# find tray window of the app
$target = [IntPtr]::Zero
$procs = Get-Process opendown -ErrorAction SilentlyContinue
if (-not $procs) { Write-Host "NO_PROC"; exit 1 }
$pids = @($procs | ForEach-Object { $_.Id })
$cb = [Sim+EnumProc]{
  param($h,$l)
  $pid2 = 0
  [void][Sim]::GetWindowThreadProcessId($h, [ref]$pid2)
  if ($pids -contains $pid2) {
    $sb = New-Object System.Text.StringBuilder 256
    [void][Sim]::GetClassName($h, $sb, 256)
    if ($sb.ToString() -eq "tray_icon_app") { $script:target = $h; return $false }
  }
  return $true
}
[void][Sim]::EnumWindows($cb, [IntPtr]::Zero)
if ($target -eq [IntPtr]::Zero) { Write-Host "TRAYWND_NOT_FOUND"; exit 2 }
Write-Host ("tray window: " + $target)
$ok = Ensure-Flyout
if (-not $ok) { Write-Host "FLYOUT_FAIL" }
Start-Sleep -Milliseconds 400
if ($Action -eq "exit") {
  [void][Sim]::SetCursorPos(960, 540)
  Start-Sleep -Milliseconds 200
  [void][Sim]::PostMessage($target, 6002, [IntPtr]::Zero, [IntPtr]0x0205)
  Write-Host "WM_RBUTTONUP posted"
  Start-Sleep -Milliseconds 900
  $mh = [Sim]::FindWindow("#32768", $null)
  if ($mh -eq [IntPtr]::Zero) { Write-Host "NO_MENU_WINDOW"; exit 3 }
  $mr = New-Object Sim+RECT
  [void][Sim]::GetWindowRect($mh, [ref]$mr)
  Write-Host ("menu rect: " + $mr.L + "," + $mr.T + " -> " + $mr.R + "," + $mr.B)
  $cx = [int](($mr.L + $mr.R) / 2)
  $cy = $mr.B - 12
  Write-Host ("clicking Exit row at " + $cx + "," + $cy)
  [void][Sim]::SetCursorPos($cx, $cy)
  Start-Sleep -Milliseconds 300
  [Sim]::mouse_event(0x2,0,0,0,[IntPtr]::Zero); [Sim]::mouse_event(0x4,0,0,0,[IntPtr]::Zero)
  Write-Host "exit row clicked"
} else {
  [void][Sim]::SetCursorPos(960, 540)
  Start-Sleep -Milliseconds 150
  [void][Sim]::PostMessage($target, 6002, [IntPtr]::Zero, [IntPtr]0x0202)
  Write-Host "WM_LBUTTONUP posted"
}
