param([long]$Hwnd, [string]$Action = "top")
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WA {
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int idx);
}
"@
$h = [IntPtr]::new($Hwnd)
if ($Action -eq "top") { [void][WA]::SetWindowPos($h, [IntPtr]::new(-1), 0,0,0,0, 0x0003); Write-Output "TOPMOST set" }
if ($Action -eq "untop") { [void][WA]::SetWindowPos($h, [IntPtr]::new(-2), 0,0,0,0, 0x0003); Write-Output "TOPMOST off" }
if ($Action -eq "close") { [void][WA]::PostMessageW($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Write-Output "CLOSED" }
$r = New-Object WA+R
[void][WA]::GetWindowRect($h, [ref]$r)
Write-Output ("RECT {0},{1} {2}x{3}" -f $r.L, $r.T, ($r.Rt-$r.L), ($r.B-$r.T))
Write-Output ("EXSTYLE 0x{0:X}" -f [WA]::GetWindowLongW($h, -20))

