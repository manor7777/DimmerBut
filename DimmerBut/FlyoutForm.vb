Imports System.Drawing
Imports System.Drawing.Drawing2D
Imports System.Drawing.Text
Imports System.Threading
Imports System.Threading.Tasks
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class FlyoutForm
        Inherits Form

        Private Const CS_DROPSHADOW As Integer = &H20000
        Private Const WS_EX_TOOLWINDOW As Integer = &H80

        Private ReadOnly _controller As DimController
        Private ReadOnly _settings As AppSettings
        Private ReadOnly _openSettings As Action

        Private ReadOnly _monitors As New List(Of MonitorInfo)
        Private ReadOnly _pollTimer As New System.Windows.Forms.Timer()
        Private _anchor As Point
        Private _token As Integer
        Private _dialogOpen As Boolean
        Private _detecting As Boolean

        ' Drag & hover tracking
        Private _draggingMonitor As MonitorInfo
        Private _draggingDim As Boolean
        Private _hoverPowerMon As MonitorInfo
        Private _hoverToggle As Boolean
        Private _hoverSettings As Boolean

        Friend LastHiddenTick As Long

        Friend Sub New(controller As DimController, settings As AppSettings, openSettings As Action)
            _controller = controller
            _settings = settings
            _openSettings = openSettings

            FormBorderStyle = FormBorderStyle.None
            ShowInTaskbar = False
            TopMost = True
            StartPosition = FormStartPosition.Manual
            KeyPreview = True
            AutoScaleMode = AutoScaleMode.None
            Font = New Font("Segoe UI", 9.5F, FontStyle.Regular, GraphicsUnit.Point)
            Padding = New Padding(12)
            Text = "DimmerBut"
            BackColor = Color.FromArgb(10, 10, 14)

            SetStyle(ControlStyles.UserPaint Or ControlStyles.AllPaintingInWmPaint Or
                     ControlStyles.OptimizedDoubleBuffer Or ControlStyles.ResizeRedraw, True)
            DoubleBuffered = True

            AddHandler _controller.StateChanged, Sub(s, e) If Visible Then Invalidate()

            ' Background monitor polling timer
            AddHandler _pollTimer.Tick, AddressOf OnPollTimerTick
            ConfigurePollTimer()
        End Sub

        Private ReadOnly Property Sc As Single
            Get
                Return DeviceDpi / 96.0F
            End Get
        End Property

        Protected Overrides ReadOnly Property CreateParams As CreateParams
            Get
                Dim cp = MyBase.CreateParams
                cp.ClassStyle = cp.ClassStyle Or CS_DROPSHADOW
                cp.ExStyle = cp.ExStyle Or WS_EX_TOOLWINDOW
                Return cp
            End Get
        End Property

        Friend Sub ConfigurePollTimer()
            _pollTimer.Stop()
            If _settings.AutoPollMonitors AndAlso _settings.ShowHardwareBrightness Then
                _pollTimer.Interval = Math.Max(5, _settings.PollIntervalSeconds) * 1000
                _pollTimer.Start()
            End If
        End Sub

        Private Sub OnPollTimerTick(sender As Object, e As EventArgs)
            ' Background rescan so power-cycled monitors re-bind automatically
            _token += 1
            DetectAsync(_token)
        End Sub

        ' ---------------- show / hide ----------------

        Friend Sub ShowFlyout()
            _anchor = Cursor.Position
            ConfigurePollTimer()

            If _monitors.Count = 0 AndAlso _settings.ShowHardwareBrightness Then
                _detecting = True
            End If

            FitAndPosition()
            Show()
            NativeMethods.SetForegroundWindow(Handle)
            Activate()

            If _settings.ShowHardwareBrightness Then
                _token += 1
                DetectAsync(_token)
            End If
        End Sub

        Protected Overrides Sub OnVisibleChanged(e As EventArgs)
            MyBase.OnVisibleChanged(e)
            If Not Visible Then
                _draggingMonitor = Nothing
                _draggingDim = False
                LastHiddenTick = Environment.TickCount64
            End If
        End Sub

        Protected Overrides Sub OnDeactivate(e As EventArgs)
            MyBase.OnDeactivate(e)
            If Visible AndAlso Not _dialogOpen Then Hide()
        End Sub

        Protected Overrides Function ProcessCmdKey(ByRef msg As Message, keyData As Keys) As Boolean
            If keyData = Keys.Escape Then
                Hide()
                Return True
            End If
            Return MyBase.ProcessCmdKey(msg, keyData)
        End Function

        ' ---------------- DWM & Backdrop ----------------

        Private Sub ApplyDwm()
            Dim dark As Integer = 1
            NativeMethods.DwmSetWindowAttribute(Handle, 20, dark, 4)   ' Immersive Dark Mode
            Dim corner As Integer = 2
            NativeMethods.DwmSetWindowAttribute(Handle, 33, corner, 4) ' Rounded corners

            If _settings.FlyoutAcrylic AndAlso Environment.OSVersion.Version.Build >= 22621 Then
                Dim backdrop As Integer = 3                            ' Acrylic
                NativeMethods.DwmSetWindowAttribute(Handle, 38, backdrop, 4)
                Dim m As New NativeMethods.MARGINS With {
                    .cxLeftWidth = -1, .cxRightWidth = -1, .cyTopHeight = -1, .cyBottomHeight = -1}
                NativeMethods.DwmExtendFrameIntoClientArea(Handle, m)
            Else
                Dim backdrop As Integer = 1                            ' None
                NativeMethods.DwmSetWindowAttribute(Handle, 38, backdrop, 4)
                Dim m As New NativeMethods.MARGINS()
                NativeMethods.DwmExtendFrameIntoClientArea(Handle, m)
            End If
        End Sub

        Protected Overrides Sub OnHandleCreated(e As EventArgs)
            MyBase.OnHandleCreated(e)
            ApplyDwm()
        End Sub

        ' ---------------- Layout Geometry ----------------

        Private Sub FitAndPosition()
            Dim s = Sc
            Dim contentW As Integer = CInt(290 * s)
            Dim totalH As Integer = CInt(Padding.Top + Padding.Bottom)

            If _settings.ShowHardwareBrightness Then
                If _detecting AndAlso _monitors.Count = 0 Then
                    totalH += CInt(34 * s)
                ElseIf _monitors.Count = 0 Then
                    totalH += CInt(34 * s)
                Else
                    totalH += CInt(_monitors.Count * 58 * s)
                End If
            End If

            totalH += CInt(58 * s) ' Dim slider
            totalH += CInt(42 * s) ' Night toggle
            totalH += CInt(40 * s) ' Settings button

            ClientSize = New Size(contentW + Padding.Horizontal, totalH)

            Dim wa = Screen.FromPoint(_anchor).WorkingArea
            Dim margin As Integer = 12
            Dim x As Integer = If(_anchor.X > wa.Left + wa.Width \ 2, wa.Right - Width - margin, wa.Left + margin)
            Dim y As Integer = If(_anchor.Y > wa.Top + wa.Height \ 2, wa.Bottom - Height - margin, wa.Top + margin)
            Location = New Point(x, y)
        End Sub

        ' ---------------- Painting ----------------

        Protected Overrides Sub OnPaintBackground(e As PaintEventArgs)
            ' Light semi-translucent dark wash (Alpha 115) letting maximum wallpaper color glow through
            Using b As New SolidBrush(Color.FromArgb(115, 10, 10, 14))
                e.Graphics.FillRectangle(b, ClientRectangle)
            End Using
        End Sub

        Protected Overrides Sub OnPaint(e As PaintEventArgs)
            MyBase.OnPaint(e)
            Dim g = e.Graphics
            g.SmoothingMode = SmoothingMode.AntiAlias
            g.TextRenderingHint = TextRenderingHint.ClearTypeGridFit

            Dim s = Sc
            Dim x As Single = Padding.Left
            Dim y As Single = Padding.Top
            Dim w As Single = ClientSize.Width - Padding.Horizontal

            ' 1. Monitors
            If _settings.ShowHardwareBrightness Then
                If _detecting AndAlso _monitors.Count = 0 Then
                    DrawLabel(g, "Detecting displays...", New RectangleF(x, y, w, 32 * s))
                    y += 34 * s
                ElseIf _monitors.Count = 0 Then
                    DrawLabel(g, "No controllable displays found (DDC/CI).", New RectangleF(x, y, w, 32 * s))
                    y += 34 * s
                Else
                    For Each m In _monitors
                        Dim h = 56 * s
                        Dim showPower = _settings.ShowPowerControl AndAlso m.SupportsPower
                        Dim isPowerHover = (_hoverPowerMon Is m)
                        DrawSlider(g, m.Name, FlyoutTheme.GlyphMonitor, m.Percent,
                                   If(showPower, FlyoutTheme.GlyphPower, ""), isPowerHover,
                                   New RectangleF(x, y, w, h))
                        y += 58 * s
                    Next
                End If
            End If

            ' 2. Extra Dimming Slider
            Dim dimH = 56 * s
            DrawSlider(g, "Extra dimming", FlyoutTheme.GlyphBrightness, _controller.EffectiveLevel,
                       "", False, New RectangleF(x, y, w, dimH))
            y += 58 * s

            ' 3. Night Mode Toggle
            Dim togH = 40 * s
            Dim status = If(_controller.NightEnabled, $"{CInt(_controller.NightKelvin)}K", "Off")
            DrawToggle(g, "Night mode", FlyoutTheme.GlyphMoon, status, _controller.NightEnabled, _hoverToggle,
                       New RectangleF(x, y, w, togH))
            y += 42 * s

            ' 4. Settings Button
            Dim btnH = 38 * s
            DrawButton(g, "Settings...", FlyoutTheme.GlyphSettings, _hoverSettings, New RectangleF(x, y, w, btnH))

            ' Clean translucent outer border
            Using p As New Pen(Color.FromArgb(90, 255, 255, 255), 1.0F)
                g.DrawRectangle(p, 0, 0, Width - 1, Height - 1)
            End Using
        End Sub

        Private Sub DrawSlider(g As Graphics, caption As String, glyph As String, value As Integer,
                               actionGlyph As String, actionHover As Boolean, r As RectangleF)
            Dim s = Sc
            Dim topH As Single = 28 * s
            Dim textLeft As Single = r.X + 6 * s

            If glyph.Length > 0 AndAlso FlyoutTheme.IconFontName.Length > 0 Then
                DrawIcon(g, glyph, New RectangleF(r.X + 4 * s, r.Y, 26 * s, topH), FlyoutTheme.TextPrimary)
                textLeft = r.X + 34 * s
            End If

            Dim actionW As Single = If(actionGlyph.Length > 0, 36 * s, 0)
            Dim valueW As Single = 52 * s

            DrawText(g, caption, New RectangleF(textLeft, r.Y, r.Width - (textLeft - r.X) - actionW - valueW, topH),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
            DrawText(g, value.ToString() & "%", New RectangleF(r.Right - actionW - valueW, r.Y, valueW - 4 * s, topH),
                     FlyoutTheme.TextSecondary, StringAlignment.Far)

            If actionGlyph.Length > 0 Then
                Dim ar = New RectangleF(r.Right - 36 * s, r.Y, 34 * s, 28 * s)
                If actionHover Then
                    Using p = FlyoutTheme.RoundRect(New RectangleF(ar.X, ar.Y + 2 * s, ar.Width, ar.Height - 4 * s), 6 * s)
                        Using b As New SolidBrush(Color.FromArgb(40, 255, 255, 255))
                            g.FillPath(b, p)
                        End Using
                    End Using
                End If
                DrawIcon(g, actionGlyph, ar, FlyoutTheme.TextPrimary)
            End If

            Dim x0 As Single = r.X + 14 * s
            Dim x1 As Single = r.Right - 14 * s
            Dim ty As Single = r.Y + 42 * s
            Dim th As Single = 4 * s
            Dim frac As Single = Math.Clamp(value / 100.0F, 0.0F, 1.0F)
            Dim tx As Single = x0 + frac * (x1 - x0)

            Using p = FlyoutTheme.RoundRect(New RectangleF(x0, ty - th / 2, x1 - x0, th), th / 2)
                Using b As New SolidBrush(FlyoutTheme.TrackColor)
                    g.FillPath(b, p)
                End Using
            End Using

            If tx > x0 Then
                Using p = FlyoutTheme.RoundRect(New RectangleF(x0, ty - th / 2, tx - x0, th), th / 2)
                    Using b As New SolidBrush(FlyoutTheme.Accent)
                        g.FillPath(b, p)
                    End Using
                End Using
            End If

            Dim ro As Single = 10 * s
            Dim ri As Single = 6 * s
            Using b As New SolidBrush(Color.FromArgb(255, 240, 240, 245))
                g.FillEllipse(b, tx - ro, ty - ro, ro * 2, ro * 2)
            End Using
            Using b As New SolidBrush(FlyoutTheme.Accent)
                g.FillEllipse(b, tx - ri, ty - ri, ri * 2, ri * 2)
            End Using
        End Sub

        Private Sub DrawToggle(g As Graphics, caption As String, glyph As String, statusText As String,
                               isChecked As Boolean, isHover As Boolean, r As RectangleF)
            Dim s = Sc
            If isHover Then
                Using p = FlyoutTheme.RoundRect(New RectangleF(r.X, r.Y + 2 * s, r.Width, r.Height - 4 * s), 6 * s)
                    Using b As New SolidBrush(Color.FromArgb(20, 255, 255, 255))
                        g.FillPath(b, p)
                    End Using
                End Using
            End If

            Dim textLeft As Single = r.X + 6 * s
            If glyph.Length > 0 AndAlso FlyoutTheme.IconFontName.Length > 0 Then
                DrawIcon(g, glyph, New RectangleF(r.X + 4 * s, r.Y, 26 * s, r.Height), FlyoutTheme.TextPrimary)
                textLeft = r.X + 34 * s
            End If

            Dim swW As Single = 40 * s
            Dim swH As Single = 20 * s
            Dim swX As Single = r.Right - swW - 12 * s
            Dim swY As Single = r.Y + (r.Height - swH) / 2

            DrawText(g, caption, New RectangleF(textLeft, r.Y, swX - textLeft - 70 * s, r.Height),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
            DrawText(g, statusText, New RectangleF(swX - 70 * s, r.Y, 64 * s, r.Height),
                     FlyoutTheme.TextSecondary, StringAlignment.Far)

            Using p = FlyoutTheme.RoundRect(New RectangleF(swX, swY, swW, swH), swH / 2)
                If isChecked Then
                    Using b As New SolidBrush(FlyoutTheme.Accent)
                        g.FillPath(b, p)
                    End Using
                Else
                    Using pen As New Pen(Color.FromArgb(255, 120, 120, 130), 1.5F * s)
                        g.DrawPath(pen, p)
                    End Using
                End If
            End Using

            Dim kr As Single = If(isChecked, 6.5F, 5.5F) * s
            Dim kx As Single = If(isChecked, swX + swW - 10 * s, swX + 10 * s)
            Dim kc As Color = If(isChecked, Color.FromArgb(255, 18, 18, 22), Color.FromArgb(255, 210, 210, 220))
            Using b As New SolidBrush(kc)
                g.FillEllipse(b, kx - kr, swY + swH / 2 - kr, kr * 2, kr * 2)
            End Using
        End Sub

        Private Sub DrawButton(g As Graphics, text As String, glyph As String, isHover As Boolean, r As RectangleF)
            Dim s = Sc
            If isHover Then
                Using p = FlyoutTheme.RoundRect(New RectangleF(r.X, r.Y + 2 * s, r.Width, r.Height - 4 * s), 6 * s)
                    Using b As New SolidBrush(Color.FromArgb(25, 255, 255, 255))
                        g.FillPath(b, p)
                    End Using
                End Using
            End If

            Dim textLeft As Single = r.X + 6 * s
            If glyph.Length > 0 AndAlso FlyoutTheme.IconFontName.Length > 0 Then
                DrawIcon(g, glyph, New RectangleF(r.X + 4 * s, r.Y, 26 * s, r.Height), FlyoutTheme.TextPrimary)
                textLeft = r.X + 34 * s
            End If

            DrawText(g, text, New RectangleF(textLeft, r.Y, r.Width - (textLeft - r.X), r.Height),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
        End Sub

        Private Sub DrawLabel(g As Graphics, text As String, r As RectangleF)
            DrawText(g, text, New RectangleF(r.X + 6 * Sc, r.Y, r.Width - 12 * Sc, r.Height),
                     FlyoutTheme.TextSecondary, StringAlignment.Near)
        End Sub

        Private Sub DrawText(g As Graphics, text As String, rect As RectangleF, color As Color,
                               align As StringAlignment)
            If String.IsNullOrEmpty(text) Then Return
            Using sf As New StringFormat With {
                    .Alignment = align,
                    .LineAlignment = StringAlignment.Center,
                    .Trimming = StringTrimming.EllipsisCharacter,
                    .FormatFlags = StringFormatFlags.NoWrap}
                Using br As New SolidBrush(color)
                    g.DrawString(text, Font, br, rect, sf)
                End Using
            End Using
        End Sub

        Private Sub DrawIcon(g As Graphics, glyph As String, rect As RectangleF, color As Color)
            If String.IsNullOrEmpty(glyph) OrElse FlyoutTheme.IconFontName.Length = 0 Then Return
            Using f As New Font(FlyoutTheme.IconFontName, 12.0F, FontStyle.Regular, GraphicsUnit.Point)
                Using sf As New StringFormat With {
                        .Alignment = StringAlignment.Center,
                        .LineAlignment = StringAlignment.Center}
                    Using br As New SolidBrush(color)
                        g.DrawString(glyph, f, br, rect, sf)
                    End Using
                End Using
            End Using
        End Sub

        ' ---------------- Hit Testing & Mouse Interactions ----------------

        Private Function GetItemRects() As (Monitors As List(Of (Mon As MonitorInfo, Rect As RectangleF, ActionRect As RectangleF)),
                                           DimSlider As RectangleF,
                                           NightToggle As RectangleF,
                                           SettingsBtn As RectangleF)
            Dim s = Sc
            Dim x As Single = Padding.Left
            Dim y As Single = Padding.Top
            Dim w As Single = ClientSize.Width - Padding.Horizontal

            Dim monList As New List(Of (Mon As MonitorInfo, Rect As RectangleF, ActionRect As RectangleF))
            If _settings.ShowHardwareBrightness Then
                If _detecting AndAlso _monitors.Count = 0 Then
                    y += 34 * s
                ElseIf _monitors.Count = 0 Then
                    y += 34 * s
                Else
                    For Each m In _monitors
                        Dim r As New RectangleF(x, y, w, 56 * s)
                        Dim ar As New RectangleF(r.Right - 36 * s, r.Y, 34 * s, 28 * s)
                        monList.Add((m, r, ar))
                        y += 58 * s
                    Next
                End If
            End If

            Dim dimR As New RectangleF(x, y, w, 56 * s)
            y += 58 * s

            Dim togR As New RectangleF(x, y, w, 40 * s)
            y += 42 * s

            Dim btnR As New RectangleF(x, y, w, 38 * s)

            Return (monList, dimR, togR, btnR)
        End Function

        Private Sub UpdateSliderFromX(x As Single, r As RectangleF, ByRef outVal As Integer, maxVal As Integer)
            Dim s = Sc
            Dim x0 As Single = r.X + 14 * s
            Dim x1 As Single = r.Right - 14 * s
            Dim frac As Single = Math.Clamp((x - x0) / (x1 - x0), 0.0F, 1.0F)
            outVal = CInt(Math.Round(frac * maxVal))
        End Sub

        Protected Overrides Sub OnMouseDown(e As MouseEventArgs)
            MyBase.OnMouseDown(e)
            If e.Button <> MouseButtons.Left Then Return

            Dim rects = GetItemRects()

            ' Check Monitor Sliders & Power Buttons
            For Each item In rects.Monitors
                If _settings.ShowPowerControl AndAlso item.Mon.SupportsPower AndAlso item.ActionRect.Contains(e.Location) Then
                    TurnOffMonitor(item.Mon)
                    Return
                End If
                If item.Rect.Contains(e.Location) AndAlso e.Y >= item.Rect.Y + 20 * Sc Then
                    _draggingMonitor = item.Mon
                    Dim v As Integer
                    UpdateSliderFromX(e.X, item.Rect, v, 100)
                    item.Mon.RequestBrightness(v)
                    Invalidate()
                    Return
                End If
            Next

            ' Check Extra Dimming Slider
            If rects.DimSlider.Contains(e.Location) AndAlso e.Y >= rects.DimSlider.Y + 20 * Sc Then
                _draggingDim = True
                Dim v As Integer
                UpdateSliderFromX(e.X, rects.DimSlider, v, AppSettings.MaxLevel)
                _controller.Level = v
                Invalidate()
                Return
            End If

            ' Check Night Mode Toggle
            If rects.NightToggle.Contains(e.Location) Then
                If Not _controller.NightEnabled AndAlso Not _settings.HasLocation Then
                    Hide()
                    _openSettings()
                    Return
                End If
                _controller.NightEnabled = Not _controller.NightEnabled
                Invalidate()
                Return
            End If

            ' Check Settings Button
            If rects.SettingsBtn.Contains(e.Location) Then
                Hide()
                _openSettings()
                Return
            End If
        End Sub

        Protected Overrides Sub OnMouseMove(e As MouseEventArgs)
            MyBase.OnMouseMove(e)
            Dim rects = GetItemRects()

            If _draggingMonitor IsNot Nothing Then
                For Each item In rects.Monitors
                    If item.Mon Is _draggingMonitor Then
                        Dim v As Integer
                        UpdateSliderFromX(e.X, item.Rect, v, 100)
                        item.Mon.RequestBrightness(v)
                        Invalidate()
                        Return
                    End If
                Next
            End If

            If _draggingDim Then
                Dim v As Integer
                UpdateSliderFromX(e.X, rects.DimSlider, v, AppSettings.MaxLevel)
                _controller.Level = v
                Invalidate()
                Return
            End If

            ' Track Hover States
            Dim newHoverPower As MonitorInfo = Nothing
            For Each item In rects.Monitors
                If _settings.ShowPowerControl AndAlso item.Mon.SupportsPower AndAlso item.ActionRect.Contains(e.Location) Then
                    newHoverPower = item.Mon
                    Exit For
                End If
            Next

            Dim newHoverTog = rects.NightToggle.Contains(e.Location)
            Dim newHoverSet = rects.SettingsBtn.Contains(e.Location)

            If newHoverPower IsNot _hoverPowerMon OrElse newHoverTog <> _hoverToggle OrElse newHoverSet <> _hoverSettings Then
                _hoverPowerMon = newHoverPower
                _hoverToggle = newHoverTog
                _hoverSettings = newHoverSet
                Cursor = If(newHoverPower IsNot Nothing OrElse newHoverTog OrElse newHoverSet, Cursors.Hand, Cursors.Default)
                Invalidate()
            End If
        End Sub

        Protected Overrides Sub OnMouseUp(e As MouseEventArgs)
            MyBase.OnMouseUp(e)
            _draggingMonitor = Nothing
            _draggingDim = False
            Invalidate()
        End Sub

        Protected Overrides Sub OnMouseLeave(e As EventArgs)
            MyBase.OnMouseLeave(e)
            _hoverPowerMon = Nothing
            _hoverToggle = False
            _hoverSettings = False
            Cursor = Cursors.Default
            Invalidate()
        End Sub

        Protected Overrides Sub OnMouseWheel(e As MouseEventArgs)
            MyBase.OnMouseWheel(e)
            Dim rects = GetItemRects()
            Dim delta = If(e.Delta > 0, 5, -5)

            For Each item In rects.Monitors
                If item.Rect.Contains(e.Location) Then
                    item.Mon.RequestBrightness(item.Mon.Percent + delta)
                    Invalidate()
                    Return
                End If
            Next

            If rects.DimSlider.Contains(e.Location) Then
                _controller.Level += delta
                Invalidate()
            End If
        End Sub

        ' ---------------- Monitors Background Detection ----------------

        Private Sub ReleaseMonitors()
            For Each m In _monitors
                m.Dispose()
            Next
            _monitors.Clear()
        End Sub

        Private Async Sub DetectAsync(tok As Integer)
            Dim found As List(Of MonitorInfo)
            Try
                found = Await Task.Run(Function() MonitorManager.Enumerate())
            Catch
                found = New List(Of MonitorInfo)
            End Try

            If tok <> _token Then
                For Each m In found
                    m.Dispose()
                Next
                Return
            End If

            ReleaseMonitors()
            _monitors.AddRange(found)
            _detecting = False

            If Visible Then
                FitAndPosition()
                Invalidate()
            End If
        End Sub

        ' ---------------- Direct DPM Power Off ----------------

        Private Sub TurnOffMonitor(m As MonitorInfo)
            If m Is Nothing Then Return

            If Not _settings.SuppressPowerWarning Then
                _dialogOpen = True
                Using dlg As New Form()
                    dlg.Text = "DimmerBut"
                    dlg.FormBorderStyle = FormBorderStyle.FixedDialog
                    dlg.MaximizeBox = False
                    dlg.MinimizeBox = False
                    dlg.StartPosition = FormStartPosition.CenterScreen
                    dlg.ClientSize = New Size(340, 160)
                    dlg.Font = SystemFonts.MessageBoxFont

                    DarkTheme.ApplyWindowDarkMode(dlg.Handle)
                    dlg.BackColor = DarkTheme.Background
                    dlg.ForeColor = DarkTheme.TextPrimary

                    Dim msg As New Label With {
                        .Text = "Turning off this display puts it into DPM power-save mode." &
                                Environment.NewLine & Environment.NewLine &
                                "You can turn it back on by pressing the monitor's power button. Continue?",
                        .Location = New Point(14, 14),
                        .Size = New Size(312, 60),
                        .AutoSize = False,
                        .ForeColor = DarkTheme.TextPrimary
                    }

                    Dim chk As New CheckBox With {
                        .Text = "Don't show this warning again",
                        .AutoSize = True,
                        .Location = New Point(14, 82),
                        .ForeColor = DarkTheme.TextPrimary
                    }

                    Dim btnYes As New Button With {
                        .Text = "Yes",
                        .DialogResult = DialogResult.Yes,
                        .Location = New Point(148, 116),
                        .Size = New Size(84, 30)
                    }
                    Dim btnNo As New Button With {
                        .Text = "No",
                        .DialogResult = DialogResult.No,
                        .Location = New Point(242, 116),
                        .Size = New Size(84, 30)
                    }

                    dlg.Controls.AddRange({msg, chk, btnYes, btnNo})
                    DarkTheme.StyleAll(dlg)

                    dlg.AcceptButton = btnYes
                    dlg.CancelButton = btnNo

                    Dim res = dlg.ShowDialog(Me)
                    _dialogOpen = False

                    If res = DialogResult.Yes Then
                        If chk.Checked Then
                            _settings.SuppressPowerWarning = True
                            _settings.Save()
                        End If
                    Else
                        Activate()
                        Return
                    End If
                End Using
            End If

            m.SetPower(4UI)
            Hide()
        End Sub

        Protected Overrides Sub Dispose(disposing As Boolean)
            If disposing Then
                _pollTimer.Stop()
                _pollTimer.Dispose()
                ReleaseMonitors()
            End If
            MyBase.Dispose(disposing)
        End Sub

    End Class

End Namespace
