Imports System.Runtime.InteropServices

Namespace DimmerBut

    <StructLayout(LayoutKind.Sequential)>
    Friend Structure MAGCOLOREFFECT
        <MarshalAs(UnmanagedType.ByValArray, SizeConst:=25)>
        Public Transform As Single()
    End Structure

    Friend NotInheritable Class NativeMethods

        Friend Const WM_HOTKEY As Integer = &H312

        Friend Const MOD_ALT As UInteger = &H1UI
        Friend Const MOD_CONTROL As UInteger = &H2UI
        Friend Const MOD_SHIFT As UInteger = &H4UI
        Friend Const MOD_NOREPEAT As UInteger = &H4000UI

        <StructLayout(LayoutKind.Sequential)>
        Friend Structure RECT
            Public Left As Integer
            Public Top As Integer
            Public Right As Integer
            Public Bottom As Integer
        End Structure

        <StructLayout(LayoutKind.Sequential, CharSet:=CharSet.Unicode)>
        Friend Structure MONITORINFOEX
            Public cbSize As Integer
            Public rcMonitor As RECT
            Public rcWork As RECT
            Public dwFlags As UInteger
            <MarshalAs(UnmanagedType.ByValTStr, SizeConst:=32)>
            Public szDevice As String
        End Structure

        <StructLayout(LayoutKind.Sequential, CharSet:=CharSet.Unicode)>
        Friend Structure DISPLAY_DEVICE
            Public cb As Integer
            <MarshalAs(UnmanagedType.ByValTStr, SizeConst:=32)>
            Public DeviceName As String
            <MarshalAs(UnmanagedType.ByValTStr, SizeConst:=128)>
            Public DeviceString As String
            Public StateFlags As UInteger
            <MarshalAs(UnmanagedType.ByValTStr, SizeConst:=128)>
            Public DeviceID As String
            <MarshalAs(UnmanagedType.ByValTStr, SizeConst:=128)>
            Public DeviceKey As String
        End Structure

        <StructLayout(LayoutKind.Sequential)>
        Friend Structure MARGINS
            Public cxLeftWidth As Integer
            Public cxRightWidth As Integer
            Public cyTopHeight As Integer
            Public cyBottomHeight As Integer
        End Structure

        Friend Delegate Function MonitorEnumProc(hMonitor As IntPtr, hdcMonitor As IntPtr,
                                                 ByRef lprcMonitor As RECT, dwData As IntPtr) As Boolean

        ' ---- Magnification (dimming / tint) ----
        <DllImport("Magnification.dll", SetLastError:=True)>
        Friend Shared Function MagInitialize() As Boolean
        End Function

        <DllImport("Magnification.dll", SetLastError:=True)>
        Friend Shared Function MagUninitialize() As Boolean
        End Function

        <DllImport("Magnification.dll", SetLastError:=True)>
        Friend Shared Function MagSetFullscreenColorEffect(ByRef pEffect As MAGCOLOREFFECT) As Boolean
        End Function

        ' ---- Hotkeys / misc ----
        <DllImport("user32.dll", SetLastError:=True)>
        Friend Shared Function RegisterHotKey(hWnd As IntPtr, id As Integer,
                                              fsModifiers As UInteger, vk As UInteger) As Boolean
        End Function

        <DllImport("user32.dll", SetLastError:=True)>
        Friend Shared Function UnregisterHotKey(hWnd As IntPtr, id As Integer) As Boolean
        End Function

        <DllImport("user32.dll", SetLastError:=True)>
        Friend Shared Function DestroyIcon(hIcon As IntPtr) As Boolean
        End Function

        <DllImport("user32.dll")>
        Friend Shared Function SetForegroundWindow(hWnd As IntPtr) As Boolean
        End Function

        ' ---- Monitor enumeration ----
        <DllImport("user32.dll")>
        Friend Shared Function EnumDisplayMonitors(hdc As IntPtr, lprcClip As IntPtr,
                                                   lpfnEnum As MonitorEnumProc, dwData As IntPtr) As Boolean
        End Function

        <DllImport("user32.dll", CharSet:=CharSet.Unicode, EntryPoint:="GetMonitorInfoW")>
        Friend Shared Function GetMonitorInfo(hMonitor As IntPtr, ByRef lpmi As MONITORINFOEX) As Boolean
        End Function

        <DllImport("user32.dll", CharSet:=CharSet.Unicode, EntryPoint:="EnumDisplayDevicesW")>
        Friend Shared Function EnumDisplayDevices(lpDevice As String, iDevNum As UInteger,
                                                  ByRef lpDisplayDevice As DISPLAY_DEVICE,
                                                  dwFlags As UInteger) As Boolean
        End Function

        ' ---- DDC/CI (dxva2) ----
        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor As IntPtr,
                                                                       ByRef pdwNumberOfPhysicalMonitors As UInteger) As Boolean
        End Function

        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function GetPhysicalMonitorsFromHMONITOR(hMonitor As IntPtr,
                                                               dwPhysicalMonitorArraySize As UInteger,
                                                               pPhysicalMonitorArray As IntPtr) As Boolean
        End Function

        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function DestroyPhysicalMonitor(hMonitor As IntPtr) As Boolean
        End Function

        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function GetMonitorBrightness(hMonitor As IntPtr, ByRef pdwMinimumBrightness As UInteger,
                                                    ByRef pdwCurrentBrightness As UInteger,
                                                    ByRef pdwMaximumBrightness As UInteger) As Boolean
        End Function

        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function SetMonitorBrightness(hMonitor As IntPtr, dwNewBrightness As UInteger) As Boolean
        End Function

        <DllImport("dxva2.dll", SetLastError:=True)>
        Friend Shared Function SetVCPFeature(hMonitor As IntPtr, bVCPCode As Byte, dwNewValue As UInteger) As Boolean
        End Function

        ' ---- DWM (Windows 11 backdrop) ----
        <DllImport("dwmapi.dll")>
        Friend Shared Function DwmSetWindowAttribute(hwnd As IntPtr, dwAttribute As Integer,
                                                     ByRef pvAttribute As Integer, cbAttribute As Integer) As Integer
        End Function

        <DllImport("dwmapi.dll")>
        Friend Shared Function DwmExtendFrameIntoClientArea(hwnd As IntPtr, ByRef pMarInset As MARGINS) As Integer
        End Function

    End Class

End Namespace
