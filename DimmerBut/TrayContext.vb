Imports System.Drawing
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class TrayContext
        Inherits ApplicationContext

        Private ReadOnly _settings As AppSettings
        Private ReadOnly _controller As DimController
        Private ReadOnly _hotkeys As HotkeyWindow
        Private ReadOnly _flyout As FlyoutForm
        Private ReadOnly _tray As NotifyIcon
        Private ReadOnly _menu As New ContextMenuStrip()
        Private ReadOnly _presetItems As New List(Of ToolStripMenuItem)
        Private ReadOnly _nightItem As New ToolStripMenuItem("Night mode")
        Private _icon As Icon
        Private _settingsOpen As Boolean

        Friend Sub New()
            _settings = AppSettings.Load()
            _controller = New DimController(_settings)
            _hotkeys = New HotkeyWindow(_controller, _settings)
            _flyout = New FlyoutForm(_controller, _settings, Sub() ShowSettings())
            _icon = BuildMoonIcon(_controller.EffectiveLevel)

            ' Apply dark mode renderer to context menu
            _menu.Renderer = New DarkMenuRenderer()
            _menu.ShowImageMargin = False

            Dim header As New ToolStripLabel("DimmerBut") With {
                .Font = New Font(SystemFonts.MenuFont, FontStyle.Bold),
                .ForeColor = DarkTheme.TextPrimary
            }
            _menu.Items.Add(header)
            _menu.Items.Add(New ToolStripSeparator())

            For value = 0 To AppSettings.MaxLevel Step AppSettings.Step_
                Dim v = value
                Dim item As New ToolStripMenuItem($"{v}%") With {.Tag = v}
                AddHandler item.Click, Sub() _controller.Level = v
                _presetItems.Add(item)
                _menu.Items.Add(item)
            Next

            _menu.Items.Add(New ToolStripSeparator())
            AddHandler _nightItem.Click, Sub() ToggleNight()
            _menu.Items.Add(_nightItem)
            _menu.Items.Add("Settings...", Nothing, Sub() ShowSettings())
            _menu.Items.Add("About...", Nothing, Sub() ShowAbout())
            _menu.Items.Add("Exit", Nothing, Sub() ExitThread())

            _tray = New NotifyIcon() With {
                .Icon = _icon,
                .ContextMenuStrip = _menu,
                .Visible = True
            }
            AddHandler _tray.MouseUp, AddressOf OnTrayMouseUp
            AddHandler _controller.StateChanged, AddressOf RefreshUi
            RefreshUi(Nothing, EventArgs.Empty)
        End Sub

        Private Sub ToggleNight()
            If Not _controller.NightEnabled AndAlso Not _settings.HasLocation Then
                MessageBox.Show("Night mode needs a location first. Open Settings > Night mode.",
                                "DimmerBut", MessageBoxButtons.OK, MessageBoxIcon.Information)
                ShowSettings()
                Return
            End If
            _controller.NightEnabled = Not _controller.NightEnabled
        End Sub

        ' Left-click toggles the flyout; right-click shows the menu.
        Private Sub OnTrayMouseUp(sender As Object, e As MouseEventArgs)
            If e.Button <> MouseButtons.Left Then Return
            If _flyout.Visible Then
                _flyout.Hide()
            ElseIf Environment.TickCount64 - _flyout.LastHiddenTick > 300 Then
                _flyout.ShowFlyout()
            End If
        End Sub

        Private Sub ShowSettings()
            If _settingsOpen Then Return
            _settingsOpen = True
            Try
                Using f As New SettingsForm(_settings)
                    If f.ShowDialog() = DialogResult.OK Then
                        _settings.Save()
                        AppSettings.SetStartWithWindows(f.StartWithWindows)
                        _controller.ReloadSettings()
                        Dim failures = _hotkeys.Reload()
                        If failures.Count > 0 Then
                            MessageBox.Show(String.Join(Environment.NewLine, failures),
                                            "DimmerBut", MessageBoxButtons.OK, MessageBoxIcon.Warning)
                        End If
                    End If
                End Using
            Finally
                _settingsOpen = False
            End Try
        End Sub

        Private Shared Sub ShowAbout()
            MessageBox.Show("DimmerBut" & Environment.NewLine &
                            "Software dimming below your monitor's minimum, an optional f.lux-style" & Environment.NewLine &
                            "night tint, and hardware brightness / power control for your displays." & Environment.NewLine &
                            "Dimming and tint use the Windows Magnification colour effect, so video," & Environment.NewLine &
                            "PiP, the taskbar and Start menu are covered too.",
                            "About DimmerBut", MessageBoxButtons.OK, MessageBoxIcon.Information)
        End Sub

        Private Sub RefreshUi(sender As Object, e As EventArgs)
            Dim lvl = _controller.EffectiveLevel
            For Each item In _presetItems
                item.Checked = (CInt(item.Tag) = lvl)
            Next
            _nightItem.Checked = _controller.NightEnabled

            Dim tip = $"DimmerBut - {lvl}%"
            If _controller.NightEnabled Then tip &= $" | Night {CInt(_controller.NightKelvin)}K"
            _tray.Text = tip

            Dim newIcon = BuildMoonIcon(lvl)
            _tray.Icon = newIcon
            If _icon IsNot Nothing Then _icon.Dispose()
            _icon = newIcon
        End Sub

        ' 0% dimming = full moon, 90% = thin crescent (lit side on the right).
        Private Shared Function BuildMoonIcon(level As Integer) As Icon
            Dim lit As Single = Math.Clamp(1.0F - level / 100.0F, 0.0F, 1.0F)

            Using bmp As New Bitmap(32, 32)
                Using g = Graphics.FromImage(bmp)
                    g.SmoothingMode = Drawing2D.SmoothingMode.AntiAlias
                    g.Clear(Color.Transparent)

                    Using darkBrush As New SolidBrush(Color.FromArgb(255, 45, 48, 62)),
                          litBrush As New SolidBrush(Color.FromArgb(255, 250, 244, 205)),
                          outline As New Pen(Color.FromArgb(255, 150, 150, 160), 1.0F)

                        g.FillEllipse(darkBrush, 2, 2, 28, 28)
                        g.FillPie(litBrush, 2, 2, 28, 28, -90, 180)

                        Dim rx As Single
                        If lit > 0.5F Then
                            rx = 14.0F * (2.0F * lit - 1.0F)
                            If rx >= 0.5F Then g.FillEllipse(litBrush, 16.0F - rx, 2.0F, 2.0F * rx, 28.0F)
                        ElseIf lit < 0.5F Then
                            rx = 14.0F * (1.0F - 2.0F * lit)
                            If rx >= 0.5F Then g.FillEllipse(darkBrush, 16.0F - rx, 2.0F, 2.0F * rx, 28.0F)
                        End If

                        g.DrawEllipse(outline, 2, 2, 28, 28)
                    End Using
                End Using

                Dim h = bmp.GetHicon()
                Try
                    Using tmp = Icon.FromHandle(h)
                        Return DirectCast(tmp.Clone(), Icon)
                    End Using
                Finally
                    NativeMethods.DestroyIcon(h)
                End Try
            End Using
        End Function

        Protected Overrides Sub ExitThreadCore()
            _tray.Visible = False
            _tray.Dispose()
            _flyout.Dispose()
            _hotkeys.Dispose()
            _controller.Dispose()
            If _icon IsNot Nothing Then _icon.Dispose()
            MyBase.ExitThreadCore()
        End Sub

    End Class

End Namespace
