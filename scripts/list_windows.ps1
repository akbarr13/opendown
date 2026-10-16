Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WL {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
"@
$rows = New-Object System.Collections.ArrayList
$cb = [WL+EnumProc]{
  param($h,$l)
  $len = [WL]::GetWindowTextLength($h)
  $sb = New-Object System.Text.StringBuilder 512
  [WL]::GetWindowText($h, $sb, 511) | Out-Null
  $cls = New-Object System.Text.StringBuilder 256
  [WL]::GetClassName($h, $cls, 255) | Out-Null
  $pid2 = 0
  [WL]::GetWindowThreadProcessId($h, [ref]$pid2) | Out-Null
  $vis = [WL]::IsWindowVisible($h)
  if ($len -gt 0) {
    [void]$rows.Add([pscustomobject]@{PID=$pid2; VIS=$vis; CLS=$cls.ToString(); T=$sb.ToString()})
  }
  return $true
}
[WL]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
$rows | Where-Object { $_.T -match "OpenDown|egui|winit" -or $_.CLS -match "Window Class|Chrome_WidgetWin" } | ForEach-Object {
  Write-Output ("PID={0} VIS={1} CLS='{2}' T='{3}'" -f $_.PID, $_.VIS, $_.CLS, $_.T)
}
Write-Output "---- top visible windows ----"
$rows | Where-Object { $_.VIS } | Select-Object -First 25 | ForEach-Object {
  Write-Output ("PID={0} CLS='{1}' T='{2}'" -f $_.PID, $_.CLS, $_.T)
}
