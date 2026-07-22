<#
.SYNOPSIS
  Probe the UI Automation tree of the main window
#>
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class WH3 {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextLengthW(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();
    public static List<IntPtr> FindByTitle(string t) {
        var r = new List<IntPtr>();
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h) || h == GetShellWindow()) return true;
            int len = GetWindowTextLengthW(h);
            if (len <= 0) return true;
            var sb = new System.Text.StringBuilder(len + 1);
            GetWindowTextW(h, sb, sb.Capacity);
            if (sb.ToString().IndexOf(t, StringComparison.OrdinalIgnoreCase) >= 0) r.Add(h);
            return true;
        }, IntPtr.Zero);
        return r;
    }
}
"@

$wins = [WH3]::FindByTitle("Palworld Server Manager")
if ($wins.Count -eq 0) {
    Write-Host "NOT FOUND"
    exit 1
}

$hwnd = $wins[0]
Write-Host ("HWND: 0x{0:x}" -f $hwnd.ToInt64())

$el = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
Write-Host ("Root: {0} / Name='{1}' / Class='{2}'" -f $el.Current.ControlType.ProgrammaticName, $el.Current.Name, $el.Current.ClassName)

$all = $el.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
Write-Host ("Total descendants: {0}" -f $all.Count)

$i = 0
foreach ($e in $all) {
    $name = $e.Current.Name
    $cls = $e.Current.ClassName
    $ct = $e.Current.ControlType.ProgrammaticName
    $autoId = $e.Current.AutomationId
    if ($name -or $cls -or $autoId) {
        Write-Host ("  [{0}] {1} cls='{2}' name='{3}' autoId='{4}'" -f $i, $ct, $cls, $name, $autoId)
    }
    $i++
    if ($i -ge 100) { break }
}
