Imports System.Management
Imports System.Runtime.InteropServices
Imports System.Threading

Namespace DimmerBut

    Friend Enum MonitorKind
        Ddc
        Wmi
    End Enum

    Friend NotInheritable Class MonitorInfo
        Implements IDisposable

        Friend ReadOnly Kind As MonitorKind
        Friend Property Name As String
        Friend Property Tag As String = ""
        Friend Property Percent As Integer

        Private ReadOnly _handle As IntPtr
        Private ReadOnly _min As UInteger
        Private ReadOnly _max As UInteger
        Private ReadOnly _gate As New Object()
        Private ReadOnly _timer As Timer
        Private _pending As Integer = -1
        Private _disposed As Boolean

        Friend Sub New(kind As MonitorKind, name As String, percent As Integer,
                       handle As IntPtr, minV As UInteger, maxV As UInteger)
            Me.Kind = kind
            Me.Name = name
            Me.Percent = percent
            _handle = handle
            _min = minV
            _max = maxV
            _timer = New Timer(AddressOf Flush, Nothing, Timeout.Infinite, Timeout.Infinite)
        End Sub

        Friend ReadOnly Property SupportsPower As Boolean
            Get
                Return Kind = MonitorKind.Ddc
            End Get
        End Property

        ' Update local state immediately for UI responsiveness, debounce physical hardware write
        Friend Sub RequestBrightness(percent As Integer)
            If _disposed Then Return
            Dim p = Math.Clamp(percent, 0, 100)
            Me.Percent = p
            SyncLock _gate
                _pending = p
            End SyncLock
            Try
                _timer.Change(120, Timeout.Infinite)
            Catch
            End Try
        End Sub

        Private Sub Flush(state As Object)
            Dim v As Integer
            SyncLock _gate
                v = _pending
                _pending = -1
            End SyncLock
            If v >= 0 Then WriteNow(v)
        End Sub

        Private Sub WriteNow(percent As Integer)
            Try
                If Kind = MonitorKind.Ddc Then
                    Dim raw As UInteger = CUInt(_min + Math.Round(CDbl(_max - _min) * percent / 100.0))
                    NativeMethods.SetMonitorBrightness(_handle, raw)
                Else
                    MonitorManager.SetWmiBrightness(percent)
                End If
            Catch
            End Try
        End Sub

        Friend Sub SetPower(mode As UInteger)
            If Kind <> MonitorKind.Ddc OrElse _disposed Then Return
            Try
                NativeMethods.SetVCPFeature(_handle, CByte(&HD6), mode)
            Catch
            End Try
        End Sub

        Public Sub Dispose() Implements IDisposable.Dispose
            If _disposed Then Return
            _disposed = True
            _timer.Dispose()

            Dim v As Integer
            SyncLock _gate
                v = _pending
                _pending = -1
            End SyncLock
            If v >= 0 Then WriteNow(v)

            If Kind = MonitorKind.Ddc AndAlso _handle <> IntPtr.Zero Then
                NativeMethods.DestroyPhysicalMonitor(_handle)
            End If
        End Sub

    End Class

    Friend NotInheritable Class MonitorManager

        Friend Shared Function Enumerate() As List(Of MonitorInfo)
            Dim result As New List(Of MonitorInfo)

            Dim cb As NativeMethods.MonitorEnumProc =
                Function(hMon As IntPtr, hdc As IntPtr, ByRef rc As NativeMethods.RECT, data As IntPtr) As Boolean
                    Try
                        AddFromHMonitor(hMon, result)
                    Catch
                    End Try
                    Return True
                End Function

            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, cb, IntPtr.Zero)
            GC.KeepAlive(cb)

            Dim counts As New Dictionary(Of String, Integer)
            For Each m In result
                counts(m.Name) = If(counts.ContainsKey(m.Name), counts(m.Name) + 1, 1)
            Next
            For Each m In result
                If counts(m.Name) > 1 AndAlso m.Tag.Length > 0 Then m.Name = $"{m.Name} ({m.Tag})"
            Next

            Try
                AddWmi(result)
            Catch
            End Try

            Return result
        End Function

        Private Shared Sub AddFromHMonitor(hMon As IntPtr, result As List(Of MonitorInfo))
            Dim mi As New NativeMethods.MONITORINFOEX With {
                .cbSize = Marshal.SizeOf(GetType(NativeMethods.MONITORINFOEX)),
                .szDevice = ""
            }
            Dim device As String = ""
            If NativeMethods.GetMonitorInfo(hMon, mi) Then device = mi.szDevice

            Dim friendly As String = ""
            If device.Length > 0 Then
                Dim dd As New NativeMethods.DISPLAY_DEVICE With {
                    .cb = Marshal.SizeOf(GetType(NativeMethods.DISPLAY_DEVICE)),
                    .DeviceName = "", .DeviceString = "", .DeviceID = "", .DeviceKey = ""
                }
                If NativeMethods.EnumDisplayDevices(device, 0UI, dd, 0UI) Then friendly = dd.DeviceString
            End If
            Dim tag As String = device.Replace("\", "").Replace(".", "")

            Dim count As UInteger = 0UI
            If Not NativeMethods.GetNumberOfPhysicalMonitorsFromHMONITOR(hMon, count) OrElse count = 0UI Then Return

            Dim entrySize As Integer = IntPtr.Size + 256
            Dim buf As IntPtr = Marshal.AllocHGlobal(CInt(count) * entrySize)
            Try
                If Not NativeMethods.GetPhysicalMonitorsFromHMONITOR(hMon, count, buf) Then Return

                For i As Integer = 0 To CInt(count) - 1
                    Dim h As IntPtr = Marshal.ReadIntPtr(buf, i * entrySize)
                    Dim desc As String = Marshal.PtrToStringUni(IntPtr.Add(buf, i * entrySize + IntPtr.Size))

                    Dim minV As UInteger = 0UI, curV As UInteger = 0UI, maxV As UInteger = 0UI
                    If NativeMethods.GetMonitorBrightness(h, minV, curV, maxV) AndAlso maxV > minV Then
                        Dim pct As Integer = CInt(Math.Round((curV - minV) * 100.0 / (maxV - minV)))
                        Dim nm As String = If(friendly.Length > 0, friendly, If(desc, "Display"))
                        result.Add(New MonitorInfo(MonitorKind.Ddc, nm, Math.Clamp(pct, 0, 100), h, minV, maxV) With {.Tag = tag})
                    Else
                        NativeMethods.DestroyPhysicalMonitor(h)
                    End If
                Next
            Finally
                Marshal.FreeHGlobal(buf)
            End Try
        End Sub

        Private Shared Sub AddWmi(result As List(Of MonitorInfo))
            Using searcher As New ManagementObjectSearcher("root\WMI", "SELECT * FROM WmiMonitorBrightness")
                For Each o As ManagementObject In searcher.Get()
                    Dim cur As Integer = Convert.ToInt32(o("CurrentBrightness"))
                    result.Add(New MonitorInfo(MonitorKind.Wmi, "Built-in display", Math.Clamp(cur, 0, 100),
                                               IntPtr.Zero, 0UI, 100UI))
                    Exit For
                Next
            End Using
        End Sub

        Friend Shared Sub SetWmiBrightness(percent As Integer)
            Using searcher As New ManagementObjectSearcher("root\WMI", "SELECT * FROM WmiMonitorBrightnessMethods")
                For Each o As ManagementObject In searcher.Get()
                    o.InvokeMethod("WmiSetBrightness", New Object() {CUInt(1), CByte(Math.Clamp(percent, 0, 100))})
                Next
            End Using
        End Sub

    End Class

End Namespace
