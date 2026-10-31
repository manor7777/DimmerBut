Imports System.Windows.Forms
Imports Microsoft.Win32

Namespace DimmerBut

    ' Applies dimming and night-mode tint together as ONE colour matrix using the
    ' Windows Magnification API full-screen colour effect (works on video/MPO planes,
    ' taskbar and Start menu). Each channel gain = nightGain(channel) * dimFactor.
    Friend NotInheritable Class DimController
        Implements IDisposable

        Private ReadOnly _settings As AppSettings
        Private ReadOnly _timer As New Timer With {.Interval = 2000}
        Private _level As Integer
        Private _magReady As Boolean
        Private _kelvin As Double
        Private _lastRaisedKelvin As Double

        Friend Event StateChanged As EventHandler

        Friend Sub New(settings As AppSettings)
            _settings = settings
            _level = Math.Clamp(settings.StartupLevel, 0, AppSettings.MaxLevel)
            AddHandler _timer.Tick, Sub(s, e) Apply()
            AddHandler SystemEvents.DisplaySettingsChanged, Sub(s, e) Apply()
            AddHandler SystemEvents.SessionSwitch, Sub(s, e) Apply()
            AddHandler SystemEvents.PowerModeChanged, Sub(s, e) Apply()
            _timer.Start()
            Apply()
        End Sub

        Friend Property Level As Integer
            Get
                Return _level
            End Get
            Set(value As Integer)
                Dim v = Math.Clamp(value, 0, AppSettings.MaxLevel)
                Dim changed As Boolean = (v <> _level)
                _level = v
                If v > 0 AndAlso Not _settings.DimmingEnabled Then
                    _settings.DimmingEnabled = True
                    _settings.Save()
                    changed = True
                End If
                If changed Then
                    Apply()
                    RaiseEvent StateChanged(Me, EventArgs.Empty)
                End If
            End Set
        End Property

        Friend ReadOnly Property EffectiveLevel As Integer
            Get
                Return If(_settings.DimmingEnabled, _level, 0)
            End Get
        End Property

        Friend Property NightEnabled As Boolean
            Get
                Return _settings.NightEnabled
            End Get
            Set(value As Boolean)
                If value = _settings.NightEnabled Then Return
                _settings.NightEnabled = value
                _settings.Save()
                Apply()
                RaiseEvent StateChanged(Me, EventArgs.Empty)
            End Set
        End Property

        ' Current night-mode colour temperature, or 0 when night mode is off.
        Friend ReadOnly Property NightKelvin As Double
            Get
                Return If(_settings.NightEnabled, _kelvin, 0.0)
            End Get
        End Property

        Friend Sub ReloadSettings()
            Apply()
            RaiseEvent StateChanged(Me, EventArgs.Empty)
        End Sub

        Private Shared Function BuildMatrix(r As Single, g As Single, b As Single) As MAGCOLOREFFECT
            Dim m As New MAGCOLOREFFECT With {.Transform = New Single(24) {}}
            m.Transform(0) = r
            m.Transform(6) = g
            m.Transform(12) = b
            m.Transform(18) = 1.0F
            m.Transform(24) = 1.0F
            Return m
        End Function

        Private Sub Apply()
            Dim dimF As Single = 1.0F
            If _settings.DimmingEnabled AndAlso _level > 0 Then dimF = 1.0F - _level / 100.0F

            Dim gains As Single() = {1.0F, 1.0F, 1.0F}
            If _settings.NightEnabled Then
                _kelvin = NightMode.GetKelvin(_settings, Date.Now)
                gains = NightMode.GetGains(_kelvin)
            Else
                _kelvin = 0.0
            End If

            Dim r = gains(0) * dimF
            Dim g = gains(1) * dimF
            Dim b = gains(2) * dimF
            Dim active As Boolean = (r < 0.9999F OrElse g < 0.9999F OrElse b < 0.9999F)

            If Not _magReady Then
                If Not active Then Return
                _magReady = NativeMethods.MagInitialize()
                If Not _magReady Then Return
            End If

            Dim fx = BuildMatrix(r, g, b)
            NativeMethods.MagSetFullscreenColorEffect(fx)

            If Not active Then
                NativeMethods.MagUninitialize()
                _magReady = False
            End If

            If _settings.NightEnabled AndAlso Math.Abs(_kelvin - _lastRaisedKelvin) >= 25.0 Then
                _lastRaisedKelvin = _kelvin
                RaiseEvent StateChanged(Me, EventArgs.Empty)
            End If
        End Sub

        Public Sub Dispose() Implements IDisposable.Dispose
            _timer.Stop()
            _timer.Dispose()
            If _magReady Then
                Dim fx = BuildMatrix(1.0F, 1.0F, 1.0F)
                NativeMethods.MagSetFullscreenColorEffect(fx)
                NativeMethods.MagUninitialize()
                _magReady = False
            End If
        End Sub

    End Class

End Namespace
