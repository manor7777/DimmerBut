Imports System.Drawing
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class SettingsForm
        Inherits Form

        Private ReadOnly _settings As AppSettings

        ' Tab buttons (sidebar navigation)
        Private ReadOnly _pnlNav As New Panel()
        Private ReadOnly _btnDim As New Button()
        Private ReadOnly _btnNight As New Button()
        Private ReadOnly _btnFlyout As New Button()
        Private ReadOnly _btnHotkeys As New Button()
        Private ReadOnly _navButtons As New List(Of Button)

        ' Tab content panels
        Private ReadOnly _pnlContent As New Panel()
        Private ReadOnly _pnlDim As New Panel()
        Private ReadOnly _pnlNight As New Panel()
        Private ReadOnly _pnlFlyout As New Panel()
        Private ReadOnly _pnlHotkeys As New Panel()

        ' Dimming controls
        Private ReadOnly _dimEnabled As New CheckBox()
        Private ReadOnly _startup As New NumericUpDown()
        Private ReadOnly _runAtLogon As New CheckBox()

        ' Night mode controls
        Private ReadOnly _nightEnabled As New CheckBox()
        Private ReadOnly _dayK As New NumericUpDown()
        Private ReadOnly _sunK As New NumericUpDown()
        Private ReadOnly _bedK As New NumericUpDown()
        Private ReadOnly _dayName As New Label()
        Private ReadOnly _sunName As New Label()
        Private ReadOnly _bedName As New Label()
        Private ReadOnly _wake As New DateTimePicker()
        Private ReadOnly _trans As New NumericUpDown()
        Private ReadOnly _city As New TextBox()
        Private ReadOnly _find As New Button()
        Private ReadOnly _locLabel As New Label()
        Private ReadOnly _lat As New NumericUpDown()
        Private ReadOnly _lon As New NumericUpDown()

        ' Flyout controls
        Private ReadOnly _showHw As New CheckBox()
        Private ReadOnly _showPower As New CheckBox()
        Private ReadOnly _acrylic As New CheckBox()
        Private ReadOnly _suppressPowerWarn As New CheckBox()
        Private ReadOnly _autoPoll As New CheckBox()
        Private ReadOnly _pollInterval As New NumericUpDown()

        ' Hotkeys controls
        Private ReadOnly _hotkeysOn As New CheckBox()
        Private ReadOnly _incBox As New HotkeyBox()
        Private ReadOnly _decBox As New HotkeyBox()

        Private ReadOnly _ok As New Button()
        Private ReadOnly _cancel As New Button()

        Private _hasLoc As Boolean
        Private _locName As String = ""
        Private _suppress As Boolean

        Private Shared ReadOnly Presets As (K As Integer, Name As String)() = {
            (1200, "Ember"), (1900, "Candle"), (2300, "Dim incandescent"), (2700, "Incandescent"),
            (3400, "Halogen"), (4200, "Fluorescent"), (5500, "Sunlight"), (6500, "Daylight")}

        Friend ReadOnly Property StartWithWindows As Boolean
            Get
                Return _runAtLogon.Checked
            End Get
        End Property

        Friend Sub New(settings As AppSettings)
            _settings = settings

            Text = "DimmerBut Settings"
            FormBorderStyle = FormBorderStyle.FixedDialog
            MaximizeBox = False
            MinimizeBox = False
            ShowInTaskbar = False
            StartPosition = FormStartPosition.CenterScreen
            AutoScaleMode = AutoScaleMode.Dpi
            Font = SystemFonts.MessageBoxFont
            ClientSize = New Size(490, 480)
            BackColor = DarkTheme.Background
            ForeColor = DarkTheme.TextPrimary

            ' Setup Navigation Sidebar
            _pnlNav.Location = New Point(8, 8)
            _pnlNav.Size = New Size(110, 410)
            _pnlNav.BackColor = DarkTheme.CardBackground

            SetupNavButton(_btnDim, "Dimming", 8, AddressOf ShowDimTab)
            SetupNavButton(_btnNight, "Night mode", 48, AddressOf ShowNightTab)
            SetupNavButton(_btnFlyout, "Flyout", 88, AddressOf ShowFlyoutTab)
            SetupNavButton(_btnHotkeys, "Hotkeys", 128, AddressOf ShowHotkeysTab)
            _navButtons.AddRange({_btnDim, _btnNight, _btnFlyout, _btnHotkeys})
            _pnlNav.Controls.AddRange({_btnDim, _btnNight, _btnFlyout, _btnHotkeys})

            ' Setup Content Container
            _pnlContent.Location = New Point(126, 8)
            _pnlContent.Size = New Size(356, 410)
            _pnlContent.BackColor = DarkTheme.Background

            SetupContentPanel(_pnlDim)
            SetupContentPanel(_pnlNight)
            SetupContentPanel(_pnlFlyout)
            SetupContentPanel(_pnlHotkeys)

            BuildDimPanel(_pnlDim)
            BuildNightPanel(_pnlNight)
            BuildFlyoutPanel(_pnlFlyout)
            BuildHotkeysPanel(_pnlHotkeys)

            _pnlContent.Controls.AddRange({_pnlDim, _pnlNight, _pnlFlyout, _pnlHotkeys})

            ' Bottom Buttons
            _ok.Text = "OK"
            _ok.DialogResult = DialogResult.OK
            _ok.Location = New Point(234, 432)
            _ok.Size = New Size(116, 34)
            _cancel.Text = "Cancel"
            _cancel.DialogResult = DialogResult.Cancel
            _cancel.Location = New Point(358, 432)
            _cancel.Size = New Size(116, 34)

            Controls.AddRange({_pnlNav, _pnlContent, _ok, _cancel})
            AcceptButton = _ok
            CancelButton = _cancel

            DarkTheme.StyleAll(Me)

            LoadFromSettings()

            AddHandler _ok.Click, AddressOf OnOk
            AddHandler _find.Click, AddressOf OnFind
            AddHandler _hotkeysOn.CheckedChanged, Sub() UpdateEnabledStates()
            AddHandler _showHw.CheckedChanged, Sub() UpdateEnabledStates()
            AddHandler _showPower.CheckedChanged, Sub() UpdateEnabledStates()
            AddHandler _autoPoll.CheckedChanged, Sub() UpdateEnabledStates()
            AddHandler _dayK.ValueChanged, Sub() _dayName.Text = PresetName(CInt(_dayK.Value))
            AddHandler _sunK.ValueChanged, Sub() _sunName.Text = PresetName(CInt(_sunK.Value))
            AddHandler _bedK.ValueChanged, Sub() _bedName.Text = PresetName(CInt(_bedK.Value))
            AddHandler _lat.ValueChanged, AddressOf OnCoordsEdited
            AddHandler _lon.ValueChanged, AddressOf OnCoordsEdited
            UpdateEnabledStates()

            ShowDimTab(Nothing, EventArgs.Empty)
        End Sub

        Private Sub SetupNavButton(btn As Button, text As String, y As Integer, handler As EventHandler)
            btn.Text = text
            btn.Location = New Point(4, y)
            btn.Size = New Size(102, 36)
            btn.FlatStyle = FlatStyle.Flat
            btn.FlatAppearance.BorderSize = 0
            btn.TextAlign = ContentAlignment.MiddleLeft
            btn.Padding = New Padding(8, 0, 0, 0)
            btn.Font = New Font(Font, FontStyle.Regular)
            btn.BackColor = DarkTheme.CardBackground
            btn.ForeColor = DarkTheme.TextSecondary
            AddHandler btn.Click, handler
        End Sub

        Private Sub SetupContentPanel(p As Panel)
            p.Dock = DockStyle.Fill
            p.BackColor = DarkTheme.Background
            p.Visible = False
        End Sub

        Private Sub SelectNav(activeBtn As Button, activePanel As Panel)
            For Each btn In _navButtons
                If btn Is activeBtn Then
                    btn.BackColor = DarkTheme.SurfaceBackground
                    btn.ForeColor = DarkTheme.Accent
                    btn.Font = New Font(Font, FontStyle.Bold)
                Else
                    btn.BackColor = DarkTheme.CardBackground
                    btn.ForeColor = DarkTheme.TextSecondary
                    btn.Font = New Font(Font, FontStyle.Regular)
                End If
            Next
            _pnlDim.Visible = (_pnlDim Is activePanel)
            _pnlNight.Visible = (_pnlNight Is activePanel)
            _pnlFlyout.Visible = (_pnlFlyout Is activePanel)
            _pnlHotkeys.Visible = (_pnlHotkeys Is activePanel)
        End Sub

        Private Sub ShowDimTab(sender As Object, e As EventArgs)
            SelectNav(_btnDim, _pnlDim)
        End Sub

        Private Sub ShowNightTab(sender As Object, e As EventArgs)
            SelectNav(_btnNight, _pnlNight)
        End Sub

        Private Sub ShowFlyoutTab(sender As Object, e As EventArgs)
            SelectNav(_btnFlyout, _pnlFlyout)
        End Sub

        Private Sub ShowHotkeysTab(sender As Object, e As EventArgs)
            SelectNav(_btnHotkeys, _pnlHotkeys)
        End Sub

        Protected Overrides Sub OnHandleCreated(e As EventArgs)
            MyBase.OnHandleCreated(e)
            DarkTheme.ApplyWindowDarkMode(Handle)
        End Sub

        ' ---------------- panel builders ----------------

        Private Sub BuildDimPanel(page As Panel)
            _dimEnabled.Text = "Enable dimming"
            _dimEnabled.AutoSize = True
            _dimEnabled.Location = New Point(10, 10)

            Dim g = MakeGroup("Startup dimming (0-90 %)", 8, 42, 62)
            _startup.Minimum = 0
            _startup.Maximum = AppSettings.MaxLevel
            _startup.Increment = AppSettings.Step_
            _startup.Location = New Point(12, 26)
            _startup.Width = 310
            g.Controls.Add(_startup)

            _runAtLogon.Text = "Start with Windows"
            _runAtLogon.AutoSize = True
            _runAtLogon.Location = New Point(10, 120)

            page.Controls.AddRange({_dimEnabled, g, _runAtLogon})
        End Sub

        Private Sub BuildNightPanel(page As Panel)
            _nightEnabled.Text = "Enable night mode (f.lux style colour tint)"
            _nightEnabled.AutoSize = True
            _nightEnabled.Location = New Point(10, 10)

            Dim gT = MakeGroup("Colour temperature (Kelvin)", 8, 38, 114)
            AddTempRow(gT, "Daytime", _dayK, _dayName, 22)
            AddTempRow(gT, "Sunset", _sunK, _sunName, 52)
            AddTempRow(gT, "Bedtime", _bedK, _bedName, 82)

            Dim lWake As New Label With {.Text = "Earliest wake time", .AutoSize = True, .Location = New Point(12, 166)}
            _wake.Format = DateTimePickerFormat.Time
            _wake.ShowUpDown = True
            _wake.Location = New Point(180, 162)
            _wake.Width = 90

            Dim lTrans As New Label With {.Text = "Transition length (min)", .AutoSize = True, .Location = New Point(12, 196)}
            _trans.Minimum = 1
            _trans.Maximum = 240
            _trans.Increment = 5
            _trans.Location = New Point(180, 192)
            _trans.Width = 90

            Dim gL = MakeGroup("Location (needed for sunrise / sunset)", 8, 226, 150)
            _city.Location = New Point(12, 24)
            _city.Width = 224
            _find.Text = "Find"
            _find.Location = New Point(242, 22)
            _find.Size = New Size(86, 27)
            _locLabel.Location = New Point(12, 56)
            _locLabel.Size = New Size(316, 38)
            _locLabel.AutoSize = False
            _locLabel.ForeColor = DarkTheme.TextSecondary

            Dim lLat As New Label With {.Text = "Lat", .AutoSize = True, .Location = New Point(12, 110)}
            _lat.DecimalPlaces = 4
            _lat.Minimum = -90D
            _lat.Maximum = 90D
            _lat.Increment = 0.1D
            _lat.Location = New Point(40, 106)
            _lat.Width = 100
            Dim lLon As New Label With {.Text = "Lon", .AutoSize = True, .Location = New Point(160, 110)}
            _lon.DecimalPlaces = 4
            _lon.Minimum = -180D
            _lon.Maximum = 180D
            _lon.Increment = 0.1D
            _lon.Location = New Point(190, 106)
            _lon.Width = 110

            gL.Controls.AddRange({_city, _find, _locLabel, lLat, _lat, lLon, _lon})

            page.Controls.AddRange({_nightEnabled, gT, lWake, _wake, lTrans, _trans, gL})
        End Sub

        Private Sub BuildFlyoutPanel(page As Panel)
            _showHw.Text = "Show monitor brightness sliders (DDC/CI and laptop panels)"
            _showHw.AutoSize = True
            _showHw.Location = New Point(10, 10)

            _showPower.Text = "Show monitor power controls"
            _showPower.AutoSize = True
            _showPower.Location = New Point(30, 36)

            _suppressPowerWarn.Text = "Don't warn when turning monitor off"
            _suppressPowerWarn.AutoSize = True
            _suppressPowerWarn.Location = New Point(50, 62)

            _autoPoll.Text = "Auto-rescan monitors periodically (for external power switches)"
            _autoPoll.AutoSize = True
            _autoPoll.Location = New Point(30, 90)

            Dim lPoll As New Label With {.Text = "Interval (sec):", .AutoSize = True, .Location = New Point(50, 118)}
            _pollInterval.Minimum = 5
            _pollInterval.Maximum = 120
            _pollInterval.Increment = 5
            _pollInterval.Location = New Point(140, 114)
            _pollInterval.Width = 60

            _acrylic.Text = "Use Windows 11 acrylic background"
            _acrylic.AutoSize = True
            _acrylic.Location = New Point(10, 146)

            Dim hint As New Label With {
                .Text = "Left-click the tray icon for the flyout; right-click for the menu.",
                .AutoSize = False,
                .Location = New Point(10, 178),
                .Size = New Size(328, 40),
                .ForeColor = DarkTheme.TextSecondary
            }

            page.Controls.AddRange({_showHw, _showPower, _suppressPowerWarn, _autoPoll, lPoll, _pollInterval, _acrylic, hint})
        End Sub

        Private Sub BuildHotkeysPanel(page As Panel)
            _hotkeysOn.Text = "Enable hotkeys"
            _hotkeysOn.AutoSize = True
            _hotkeysOn.Location = New Point(10, 10)

            Dim gInc = MakeGroup("Increase dimming hotkey", 8, 40, 62)
            _incBox.Location = New Point(12, 26)
            _incBox.Width = 310
            gInc.Controls.Add(_incBox)

            Dim gDec = MakeGroup("Decrease dimming hotkey", 8, 110, 62)
            _decBox.Location = New Point(12, 26)
            _decBox.Width = 310
            gDec.Controls.Add(_decBox)

            Dim hint As New Label With {
                .Text = "Click a box and press a combination. Delete clears it.",
                .AutoSize = False,
                .Location = New Point(10, 182),
                .Size = New Size(328, 32),
                .ForeColor = DarkTheme.TextSecondary
            }

            page.Controls.AddRange({_hotkeysOn, gInc, gDec, hint})
        End Sub

        Private Shared Function MakeGroup(title As String, x As Integer, y As Integer, height As Integer) As GroupBox
            Return New GroupBox With {.Text = title, .Location = New Point(x, y), .Size = New Size(340, height)}
        End Function

        Private Shared Sub AddTempRow(parent As GroupBox, caption As String, nud As NumericUpDown,
                                      nameLabel As Label, y As Integer)
            Dim l As New Label With {.Text = caption, .AutoSize = True, .Location = New Point(12, y + 4)}
            nud.Minimum = AppSettings.MinKelvin
            nud.Maximum = AppSettings.MaxKelvin
            nud.Increment = 100
            nud.Location = New Point(100, y)
            nud.Width = 80
            nameLabel.AutoSize = True
            nameLabel.ForeColor = DarkTheme.TextSecondary
            nameLabel.Location = New Point(192, y + 4)
            parent.Controls.AddRange({l, nud, nameLabel})
        End Sub

        Private Shared Function PresetName(k As Integer) As String
            Dim best = Presets(0)
            For Each p In Presets
                If Math.Abs(p.K - k) < Math.Abs(best.K - k) Then best = p
            Next
            Return "~ " & best.Name
        End Function

        ' ---------------- load / save ----------------

        Private Sub LoadFromSettings()
            _suppress = True

            _dimEnabled.Checked = _settings.DimmingEnabled
            _startup.Value = Math.Clamp(_settings.StartupLevel, 0, AppSettings.MaxLevel)
            _runAtLogon.Checked = AppSettings.IsStartWithWindows()

            _nightEnabled.Checked = _settings.NightEnabled
            _dayK.Value = _settings.DayKelvin
            _sunK.Value = _settings.SunsetKelvin
            _bedK.Value = _settings.BedKelvin
            _dayName.Text = PresetName(_settings.DayKelvin)
            _sunName.Text = PresetName(_settings.SunsetKelvin)
            _bedName.Text = PresetName(_settings.BedKelvin)
            _wake.Value = Date.Today.AddMinutes(_settings.WakeMinutes)
            _trans.Value = Math.Clamp(_settings.TransitionMinutes, 1, 240)

            _hasLoc = _settings.HasLocation
            _locName = _settings.LocationName
            _lat.Value = CDec(Math.Clamp(_settings.Latitude, -90.0, 90.0))
            _lon.Value = CDec(Math.Clamp(_settings.Longitude, -180.0, 180.0))
            _city.Text = _settings.LocationName
            ShowLocationLabel()

            _showHw.Checked = _settings.ShowHardwareBrightness
            _showPower.Checked = _settings.ShowPowerControl
            _suppressPowerWarn.Checked = _settings.SuppressPowerWarning
            _autoPoll.Checked = _settings.AutoPollMonitors
            _pollInterval.Value = Math.Clamp(_settings.PollIntervalSeconds, 5, 120)
            _acrylic.Checked = _settings.FlyoutAcrylic

            _hotkeysOn.Checked = _settings.HotkeysEnabled
            _incBox.SetHotkey(_settings.IncreaseMods, _settings.IncreaseKey)
            _decBox.SetHotkey(_settings.DecreaseMods, _settings.DecreaseKey)

            _suppress = False
        End Sub

        Private Sub ShowLocationLabel()
            If _hasLoc Then
                _locLabel.Text = $"{_locName}  ({_lat.Value:0.0000}, {_lon.Value:0.0000})"
            Else
                _locLabel.Text = "No location set. Type a city (e.g. ""Skegness, United Kingdom"") and press Find, or enter coordinates."
            End If
        End Sub

        Private Sub UpdateEnabledStates()
            _incBox.Enabled = _hotkeysOn.Checked
            _decBox.Enabled = _hotkeysOn.Checked
            _showPower.Enabled = _showHw.Checked
            _suppressPowerWarn.Enabled = _showHw.Checked AndAlso _showPower.Checked
            _autoPoll.Enabled = _showHw.Checked
            _pollInterval.Enabled = _showHw.Checked AndAlso _autoPoll.Checked
        End Sub

        Private Sub OnCoordsEdited(sender As Object, e As EventArgs)
            If _suppress Then Return
            _hasLoc = True
            _locName = "Custom coordinates"
            ShowLocationLabel()
        End Sub

        Private Async Sub OnFind(sender As Object, e As EventArgs)
            Dim q As String = _city.Text.Trim()
            If q.Length = 0 Then Return

            _find.Enabled = False
            _locLabel.Text = "Searching..."
            Try
                Dim res = Await Geocoder.FindAsync(q)
                If res Is Nothing Then
                    _locLabel.Text = "No match found. Try ""City, Country"" or enter coordinates."
                Else
                    _suppress = True
                    _lat.Value = CDec(Math.Round(res.Latitude, 4))
                    _lon.Value = CDec(Math.Round(res.Longitude, 4))
                    _suppress = False
                    _hasLoc = True
                    _locName = res.DisplayName
                    ShowLocationLabel()
                End If
            Catch
                _suppress = False
                _locLabel.Text = "Lookup failed (offline?). Enter latitude and longitude manually."
            Finally
                _find.Enabled = True
            End Try
        End Sub

        Private Sub InitializeComponent()

        End Sub

        Private Sub OnOk(sender As Object, e As EventArgs)
            If _nightEnabled.Checked AndAlso Not _hasLoc Then
                MessageBox.Show("Night mode needs a location. Type a city and press Find, or enter coordinates.",
                                "DimmerBut", MessageBoxButtons.OK, MessageBoxIcon.Information)
                DialogResult = DialogResult.None
                Return
            End If

            _settings.DimmingEnabled = _dimEnabled.Checked
            _settings.StartupLevel = CInt(_startup.Value)

            _settings.NightEnabled = _nightEnabled.Checked
            _settings.DayKelvin = CInt(_dayK.Value)
            _settings.SunsetKelvin = CInt(_sunK.Value)
            _settings.BedKelvin = CInt(_bedK.Value)
            _settings.WakeMinutes = _wake.Value.Hour * 60 + _wake.Value.Minute
            _settings.TransitionMinutes = CInt(_trans.Value)

            _settings.HasLocation = _hasLoc
            _settings.LocationName = _locName
            _settings.Latitude = CDbl(_lat.Value)
            _settings.Longitude = CDbl(_lon.Value)

            _settings.ShowHardwareBrightness = _showHw.Checked
            _settings.ShowPowerControl = _showPower.Checked
            _settings.SuppressPowerWarning = _suppressPowerWarn.Checked
            _settings.AutoPollMonitors = _autoPoll.Checked
            _settings.PollIntervalSeconds = CInt(_pollInterval.Value)
            _settings.FlyoutAcrylic = _acrylic.Checked

            _settings.HotkeysEnabled = _hotkeysOn.Checked
            _settings.IncreaseMods = _incBox.Mods
            _settings.IncreaseKey = _incBox.VKey
            _settings.DecreaseMods = _decBox.Mods
            _settings.DecreaseKey = _decBox.VKey
        End Sub

    End Class

End Namespace
