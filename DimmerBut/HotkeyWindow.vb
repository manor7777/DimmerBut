Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class HotkeyWindow
        Inherits NativeWindow
        Implements IDisposable

        Private Const IdIncrease As Integer = 1
        Private Const IdDecrease As Integer = 2

        Private ReadOnly _controller As DimController
        Private ReadOnly _settings As AppSettings

        Friend Sub New(controller As DimController, settings As AppSettings)
            _controller = controller
            _settings = settings
            CreateHandle(New CreateParams())
            Reload()
        End Sub

        ' Re-registers hotkeys from settings. Returns descriptions of any that failed.
        Friend Function Reload() As List(Of String)
            Dim failures As New List(Of String)
            NativeMethods.UnregisterHotKey(Handle, IdIncrease)
            NativeMethods.UnregisterHotKey(Handle, IdDecrease)
            If Not _settings.HotkeysEnabled Then Return failures

            TryRegister(IdIncrease, _settings.IncreaseMods, _settings.IncreaseKey, "Increase", failures)
            TryRegister(IdDecrease, _settings.DecreaseMods, _settings.DecreaseKey, "Decrease", failures)
            Return failures
        End Function

        Private Sub TryRegister(id As Integer, mods As UInteger, key As Integer,
                                name As String, failures As List(Of String))
            If key = 0 Then Return
            If Not NativeMethods.RegisterHotKey(Handle, id, mods Or NativeMethods.MOD_NOREPEAT, CUInt(key)) Then
                failures.Add($"{name} ({HotkeyBox.Format(mods, key)}) is already in use by another program.")
            End If
        End Sub

        Protected Overrides Sub WndProc(ByRef m As Message)
            If m.Msg = NativeMethods.WM_HOTKEY Then
                Select Case m.WParam.ToInt32()
                    Case IdIncrease
                        _controller.Level += AppSettings.Step_
                    Case IdDecrease
                        _controller.Level -= AppSettings.Step_
                End Select
            End If
            MyBase.WndProc(m)
        End Sub

        Public Sub Dispose() Implements IDisposable.Dispose
            If Handle <> IntPtr.Zero Then
                NativeMethods.UnregisterHotKey(Handle, IdIncrease)
                NativeMethods.UnregisterHotKey(Handle, IdDecrease)
                DestroyHandle()
            End If
        End Sub

    End Class

End Namespace
