Imports System.IO
Imports System.Text.Json
Imports System.Windows.Forms
Imports Microsoft.Win32

Namespace DimmerBut

    Friend NotInheritable Class AppSettings

        Friend Const MaxLevel As Integer = 90
        Friend Const Step_ As Integer = 10
        Friend Const MinKelvin As Integer = 1200
        Friend Const MaxKelvin As Integer = 6500

        ' ---- Dimming ----
        Public Property DimmingEnabled As Boolean = True
        Public Property StartupLevel As Integer = 0

        ' ---- Night mode (f.lux style) ----
        Public Property NightEnabled As Boolean = False
        Public Property DayKelvin As Integer = 6500
        Public Property SunsetKelvin As Integer = 3400
        Public Property BedKelvin As Integer = 2700
        Public Property WakeMinutes As Integer = 450
        Public Property TransitionMinutes As Integer = 60

        ' ---- Location (stored locally; looked up once by city name, or typed in) ----
        Public Property HasLocation As Boolean = False
        Public Property LocationName As String = ""
        Public Property Latitude As Double = 0.0
        Public Property Longitude As Double = 0.0

        ' ---- Flyout & Monitors ----
        Public Property ShowHardwareBrightness As Boolean = True
        Public Property ShowPowerControl As Boolean = True
        Public Property FlyoutAcrylic As Boolean = True
        Public Property SuppressPowerWarning As Boolean = False
        Public Property AutoPollMonitors As Boolean = True
        Public Property PollIntervalSeconds As Integer = 15

        ' ---- Hotkeys ----
        Public Property HotkeysEnabled As Boolean = False
        Public Property IncreaseMods As UInteger = NativeMethods.MOD_CONTROL Or NativeMethods.MOD_ALT
        Public Property IncreaseKey As Integer = Keys.Up
        Public Property DecreaseMods As UInteger = NativeMethods.MOD_CONTROL Or NativeMethods.MOD_ALT
        Public Property DecreaseKey As Integer = Keys.Down

        Private Shared ReadOnly FolderPath As String =
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "DimmerBut")
        Private Shared ReadOnly FilePath As String = Path.Combine(FolderPath, "settings.json")

        Private Const RunKey As String = "Software\Microsoft\Windows\CurrentVersion\Run"
        Private Const RunName As String = "DimmerBut"

        Friend Shared Function Load() As AppSettings
            Try
                If File.Exists(FilePath) Then
                    Dim s = JsonSerializer.Deserialize(Of AppSettings)(File.ReadAllText(FilePath))
                    If s IsNot Nothing Then
                        s.StartupLevel = Math.Clamp(s.StartupLevel, 0, MaxLevel)
                        s.DayKelvin = Math.Clamp(s.DayKelvin, MinKelvin, MaxKelvin)
                        s.SunsetKelvin = Math.Clamp(s.SunsetKelvin, MinKelvin, MaxKelvin)
                        s.BedKelvin = Math.Clamp(s.BedKelvin, MinKelvin, MaxKelvin)
                        s.WakeMinutes = Math.Clamp(s.WakeMinutes, 0, 1439)
                        s.TransitionMinutes = Math.Clamp(s.TransitionMinutes, 1, 240)
                        s.PollIntervalSeconds = Math.Clamp(s.PollIntervalSeconds, 5, 120)
                        Return s
                    End If
                End If
            Catch
            End Try
            Return New AppSettings()
        End Function

        Friend Sub Save()
            Try
                Directory.CreateDirectory(FolderPath)
                File.WriteAllText(FilePath, JsonSerializer.Serialize(Me))
            Catch
            End Try
        End Sub

        Friend Shared Function IsStartWithWindows() As Boolean
            Try
                Using k = Registry.CurrentUser.OpenSubKey(RunKey, False)
                    Return k IsNot Nothing AndAlso k.GetValue(RunName) IsNot Nothing
                End Using
            Catch
                Return False
            End Try
        End Function

        Friend Shared Sub SetStartWithWindows(enable As Boolean)
            Try
                Using k = Registry.CurrentUser.OpenSubKey(RunKey, True)
                    If k Is Nothing Then Return
                    If enable Then
                        k.SetValue(RunName, ControlChars.Quote & Environment.ProcessPath & ControlChars.Quote)
                    Else
                        k.DeleteValue(RunName, False)
                    End If
                End Using
            Catch
                Return
            End Try
        End Sub

    End Class

End Namespace
