Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W5 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
"@
$hwnds = New-Object System.Collections.ArrayList
$cb = [W5+EnumProc]{
  param($h,$l)
  $len = [W5]::GetWindowTextLength($h)
  if ($len -gt 0) {
    $sb = New-Object System.Text.StringBuilder ($len+1)
    [W5]::GetWindowText($h, $sb, $len+1) | Out-Null
    if ($sb.ToString() -like "*OpenDown*Download Manager*") { [void]$hwnds.Add($h) }
  }
  return $true
}
[W5]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
if ($hwnds.Count -eq 0) { Write-Output "NO_TARGET_WINDOW"; exit 1 }
foreach ($h in $hwnds) {
  [void][W5]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  Write-Output ("WM_CLOSE sent to " + $h)
}
