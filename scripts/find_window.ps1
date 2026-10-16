param([string]$Needle = "dst")
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class FW {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct RECT { public int L,T,R,B; }
}
"@
$found = New-Object System.Collections.ArrayList
$cb = [FW+EnumProc]{
  param($h,$l)
  $len = [FW]::GetWindowTextLength($h)
  if ($len -gt 0) {
    $sb = New-Object System.Text.StringBuilder ($len+1)
    [FW]::GetWindowText($h, $sb, $len+1) | Out-Null
    $t = $sb.ToString()
    if ($t -like "*$Needle*") {
      $r = New-Object FW+RECT
      [void][FW]::GetWindowRect($h, [ref]$r)
      [void]$found.Add(("H={0} VIS={1} POS=({2},{3}) SIZE={4}x{5} '{6}'" -f $h, [FW]::IsWindowVisible($h), $r.L, $r.T, ($r.R-$r.L), ($r.B-$r.T), $t))
    }
  }
  return $true
}
[FW]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($found.Count -eq 0) { Write-Output "NONE" } else { $found | ForEach-Object { Write-Output $_ } }

