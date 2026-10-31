Imports System.ComponentModel
Imports System.Drawing
Imports System.Drawing.Drawing2D
Imports System.Drawing.Text
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class FlyoutTheme

        Friend Shared ReadOnly Accent As Color = Color.FromArgb(255, 96, 205, 255)
        Friend Shared ReadOnly TextPrimary As Color = Color.FromArgb(255, 245, 245, 248)
        Friend Shared ReadOnly TextSecondary As Color = Color.FromArgb(255, 185, 185, 195)
        Friend Shared ReadOnly TrackColor As Color = Color.FromArgb(120, 255, 255, 255)
        Friend Shared ReadOnly BorderColor As Color = Color.FromArgb(255, 60, 60, 72)
        Friend Shared ReadOnly IconFontName As String = PickIconFont()

        Friend Shared ReadOnly GlyphMonitor As String = ChrW(&HE7F4)
        Friend Shared ReadOnly GlyphBrightness As String = ChrW(&HE706)
        Friend Shared ReadOnly GlyphMoon As String = ChrW(&HE708)
        Friend Shared ReadOnly GlyphSettings As String = ChrW(&HE713)
        Friend Shared ReadOnly GlyphPower As String = ChrW(&HE7E8)

        Private Shared Function PickIconFont() As String
            Using fc As New InstalledFontCollection()
                For Each f In fc.Families
                    If f.Name = "Segoe Fluent Icons" Then Return f.Name
                Next
                For Each f In fc.Families
                    If f.Name = "Segoe MDL2 Assets" Then Return f.Name
                Next
            End Using
            Return ""
        End Function

        Friend Shared Function RoundRect(r As RectangleF, radius As Single) As GraphicsPath
            Dim p As New GraphicsPath()
            Dim d As Single = Math.Min(radius * 2.0F, Math.Min(r.Width, r.Height))
            If d <= 0.5F Then
                p.AddRectangle(r)
                Return p
            End If
            p.AddArc(r.X, r.Y, d, d, 180, 90)
            p.AddArc(r.Right - d, r.Y, d, d, 270, 90)
            p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90)
            p.AddArc(r.X, r.Bottom - d, d, d, 90, 90)
            p.CloseFigure()
            Return p
        End Function

    End Class

    Friend MustInherit Class FlyoutControlBase
        Inherits Control

        Protected Sub New()
            SetStyle(ControlStyles.SupportsTransparentBackColor Or
                     ControlStyles.UserPaint Or ControlStyles.AllPaintingInWmPaint Or
                     ControlStyles.OptimizedDoubleBuffer Or ControlStyles.ResizeRedraw, True)
            DoubleBuffered = True
            BackColor = Color.Transparent
            Font = New Font("Segoe UI", 9.5F, FontStyle.Regular, GraphicsUnit.Point)
            Margin = New Padding(0, 1, 0, 1)
            TabStop = False
            Width = CInt(290 * Sc)
        End Sub

        Protected ReadOnly Property Sc As Single
            Get
                Return DeviceDpi / 96.0F
            End Get
        End Property

        Protected Overrides Sub OnPaintBackground(pevent As PaintEventArgs)
            ' Let parent's acrylic backdrop show through completely
        End Sub

        Protected Overrides Sub OnPaint(e As PaintEventArgs)
            If ClientRectangle.Width <= 0 OrElse ClientRectangle.Height <= 0 Then Return

            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias
            e.Graphics.TextRenderingHint = TextRenderingHint.ClearTypeGridFit
            PaintContent(e.Graphics)
        End Sub

        Protected MustOverride Sub PaintContent(g As Graphics)

        Protected Sub DrawText(g As Graphics, text As String, rect As RectangleF, color As Color,
                               align As StringAlignment, Optional fnt As Font = Nothing)
            If String.IsNullOrEmpty(text) Then Return
            Using sf As New StringFormat With {
                    .Alignment = align,
                    .LineAlignment = StringAlignment.Center,
                    .Trimming = StringTrimming.EllipsisCharacter,
                    .FormatFlags = StringFormatFlags.NoWrap}
                Using br As New SolidBrush(color)
                    g.DrawString(text, If(fnt, Font), br, rect, sf)
                End Using
            End Using
        End Sub

        Protected Sub DrawIcon(g As Graphics, glyph As String, rect As RectangleF, color As Color)
            If String.IsNullOrEmpty(glyph) OrElse FlyoutTheme.IconFontName.Length = 0 Then Return
            Using f As New Font(FlyoutTheme.IconFontName, 12.0F, FontStyle.Regular, GraphicsUnit.Point)
                DrawText(g, glyph, rect, color, StringAlignment.Center, f)
            End Using
        End Sub

        Protected ReadOnly Property HasIconFont As Boolean
            Get
                Return FlyoutTheme.IconFontName.Length > 0
            End Get
        End Property

    End Class

    ' Caption + value + direct action button (Monitor power) + slider track.
    Friend NotInheritable Class FlyoutSlider
        Inherits FlyoutControlBase

        Friend Event ValueChanged As EventHandler
        Friend Event ActionClicked As EventHandler

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Caption As String = ""

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Glyph As String = ""

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property ActionGlyph As String = ""

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Minimum As Integer = 0

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Maximum As Integer = 100

        Private _value As Integer
        Private _drag As Boolean
        Private _hoverAction As Boolean

        Friend Sub New()
            Height = CInt(56 * Sc)
        End Sub

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend ReadOnly Property Value As Integer
            Get
                Return _value
            End Get
        End Property

        Friend Sub SetValue(v As Integer, raise As Boolean)
            Dim nv = Math.Clamp(v, Minimum, Maximum)
            If nv = _value Then Return
            _value = nv
            InvalidateParent()
            If raise Then RaiseEvent ValueChanged(Me, EventArgs.Empty)
        End Sub

        Private Function ActionRect() As Rectangle
            Dim s = Sc
            Return New Rectangle(CInt(Width - 36 * s), 0, CInt(34 * s), CInt(28 * s))
        End Function

        Private Sub InvalidateParent()
            If Parent IsNot Nothing Then
                Parent.Invalidate(Bounds, True)
            Else
                Invalidate()
            End If
        End Sub

        Protected Overrides Sub PaintContent(g As Graphics)
            Dim s = Sc
            Dim topH As Single = 28 * s
            Dim textLeft As Single = 6 * s
            If HasIconFont AndAlso Glyph.Length > 0 Then
                DrawIcon(g, Glyph, New RectangleF(4 * s, 0, 26 * s, topH), FlyoutTheme.TextPrimary)
                textLeft = 34 * s
            End If

            Dim actionW As Single = If(ActionGlyph.Length > 0, 36 * s, 0)
            Dim valueW As Single = 52 * s
            DrawText(g, Caption, New RectangleF(textLeft, 0, Width - textLeft - actionW - valueW, topH),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
            DrawText(g, _value.ToString() & "%", New RectangleF(Width - actionW - valueW, 0, valueW - 4 * s, topH),
                     FlyoutTheme.TextSecondary, StringAlignment.Far)

            If ActionGlyph.Length > 0 Then
                Dim ar = ActionRect()
                If _hoverAction Then
                    Using p = FlyoutTheme.RoundRect(New RectangleF(ar.X, ar.Y + 2 * s, ar.Width, ar.Height - 4 * s), 6 * s)
                        Using b As New SolidBrush(Color.FromArgb(40, 255, 255, 255))
                            g.FillPath(b, p)
                        End Using
                    End Using
                End If
                DrawIcon(g, ActionGlyph, New RectangleF(ar.X, ar.Y, ar.Width, ar.Height), FlyoutTheme.TextPrimary)
            End If

            Dim x0 As Single = 14 * s
            Dim x1 As Single = Width - 14 * s
            Dim ty As Single = 42 * s
            Dim th As Single = 4 * s
            Dim span As Single = Math.Max(1, Maximum - Minimum)
            Dim frac As Single = (_value - Minimum) / span
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
            Dim ri As Single = If(_drag, 4 * s, 6 * s)
            Using b As New SolidBrush(Color.FromArgb(255, 240, 240, 245))
                g.FillEllipse(b, tx - ro, ty - ro, ro * 2, ro * 2)
            End Using
            Using b As New SolidBrush(FlyoutTheme.Accent)
                g.FillEllipse(b, tx - ri, ty - ri, ri * 2, ri * 2)
            End Using
        End Sub

        Private Sub UpdateFromX(x As Integer)
            Dim s = Sc
            Dim x0 As Single = 14 * s
            Dim x1 As Single = Width - 14 * s
            Dim frac As Single = Math.Clamp((x - x0) / (x1 - x0), 0.0F, 1.0F)
            SetValue(CInt(Math.Round(Minimum + frac * (Maximum - Minimum))), True)
        End Sub

        Protected Overrides Sub OnMouseDown(e As MouseEventArgs)
            MyBase.OnMouseDown(e)
            If e.Button <> MouseButtons.Left Then Return
            If ActionGlyph.Length > 0 AndAlso ActionRect().Contains(e.Location) Then
                RaiseEvent ActionClicked(Me, EventArgs.Empty)
                Return
            End If
            If e.Y >= 24 * Sc Then
                _drag = True
                Capture = True
                UpdateFromX(e.X)
            End If
        End Sub

        Protected Overrides Sub OnMouseMove(e As MouseEventArgs)
            MyBase.OnMouseMove(e)
            If _drag Then
                UpdateFromX(e.X)
                Return
            End If
            Dim h = ActionGlyph.Length > 0 AndAlso ActionRect().Contains(e.Location)
            If h <> _hoverAction Then
                _hoverAction = h
                Cursor = If(h, Cursors.Hand, Cursors.Default)
                InvalidateParent()
            End If
        End Sub

        Protected Overrides Sub OnMouseUp(e As MouseEventArgs)
            MyBase.OnMouseUp(e)
            If _drag Then
                _drag = False
                Capture = False
                InvalidateParent()
            End If
        End Sub

        Protected Overrides Sub OnMouseLeave(e As EventArgs)
            MyBase.OnMouseLeave(e)
            If _hoverAction Then
                _hoverAction = False
                Cursor = Cursors.Default
                InvalidateParent()
            End If
        End Sub

        Protected Overrides Sub OnMouseWheel(e As MouseEventArgs)
            MyBase.OnMouseWheel(e)
            SetValue(_value + If(e.Delta > 0, 5, -5), True)
        End Sub

    End Class

    ' Caption + status text + Windows 11 style switch.
    Friend NotInheritable Class FlyoutToggle
        Inherits FlyoutControlBase

        Friend Event Toggled As EventHandler

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Caption As String = ""

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Glyph As String = ""

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property StatusText As String = ""

        Private _checked As Boolean
        Private _hover As Boolean

        Friend Sub New()
            Height = CInt(40 * Sc)
        End Sub

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Checked As Boolean
            Get
                Return _checked
            End Get
            Set(value As Boolean)
                If value <> _checked Then
                    _checked = value
                    InvalidateParent()
                End If
            End Set
        End Property

        Private Sub InvalidateParent()
            If Parent IsNot Nothing Then
                Parent.Invalidate(Bounds, True)
            Else
                Invalidate()
            End If
        End Sub

        Protected Overrides Sub PaintContent(g As Graphics)
            Dim s = Sc
            If _hover Then
                Using p = FlyoutTheme.RoundRect(New RectangleF(0, 2 * s, Width, Height - 4 * s), 6 * s)
                    Using b As New SolidBrush(Color.FromArgb(20, 255, 255, 255))
                        g.FillPath(b, p)
                    End Using
                End Using
            End If

            Dim textLeft As Single = 6 * s
            If HasIconFont AndAlso Glyph.Length > 0 Then
                DrawIcon(g, Glyph, New RectangleF(4 * s, 0, 26 * s, Height), FlyoutTheme.TextPrimary)
                textLeft = 34 * s
            End If

            Dim swW As Single = 40 * s
            Dim swH As Single = 20 * s
            Dim swX As Single = Width - swW - 12 * s
            Dim swY As Single = (Height - swH) / 2

            DrawText(g, Caption, New RectangleF(textLeft, 0, swX - textLeft - 70 * s, Height),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
            DrawText(g, StatusText, New RectangleF(swX - 70 * s, 0, 64 * s, Height),
                     FlyoutTheme.TextSecondary, StringAlignment.Far)

            Using p = FlyoutTheme.RoundRect(New RectangleF(swX, swY, swW, swH), swH / 2)
                If _checked Then
                    Using b As New SolidBrush(FlyoutTheme.Accent)
                        g.FillPath(b, p)
                    End Using
                Else
                    Using pen As New Pen(Color.FromArgb(255, 120, 120, 130), 1.5F * s)
                        g.DrawPath(pen, p)
                    End Using
                End If
            End Using

            Dim kr As Single = If(_checked, 6.5F, 5.5F) * s
            Dim kx As Single = If(_checked, swX + swW - 10 * s, swX + 10 * s)
            Dim kc As Color = If(_checked, Color.FromArgb(255, 18, 18, 22), Color.FromArgb(255, 210, 210, 220))
            Using b As New SolidBrush(kc)
                g.FillEllipse(b, kx - kr, swY + swH / 2 - kr, kr * 2, kr * 2)
            End Using
        End Sub

        Protected Overrides Sub OnMouseEnter(e As EventArgs)
            MyBase.OnMouseEnter(e)
            _hover = True
            Cursor = Cursors.Hand
            InvalidateParent()
        End Sub

        Protected Overrides Sub OnMouseLeave(e As EventArgs)
            MyBase.OnMouseLeave(e)
            _hover = False
            Cursor = Cursors.Default
            InvalidateParent()
        End Sub

        Protected Overrides Sub OnMouseUp(e As MouseEventArgs)
            MyBase.OnMouseUp(e)
            If e.Button = MouseButtons.Left AndAlso ClientRectangle.Contains(e.Location) Then
                RaiseEvent Toggled(Me, EventArgs.Empty)
            End If
        End Sub

    End Class

    Friend NotInheritable Class FlyoutButton
        Inherits FlyoutControlBase

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Glyph As String = ""
        Private _hover As Boolean

        Friend Sub New()
            Height = CInt(38 * Sc)
        End Sub

        Private Sub InvalidateParent()
            If Parent IsNot Nothing Then
                Parent.Invalidate(Bounds, True)
            Else
                Invalidate()
            End If
        End Sub

        Protected Overrides Sub PaintContent(g As Graphics)
            Dim s = Sc
            If _hover Then
                Using p = FlyoutTheme.RoundRect(New RectangleF(0, 2 * s, Width, Height - 4 * s), 6 * s)
                    Using b As New SolidBrush(Color.FromArgb(25, 255, 255, 255))
                        g.FillPath(b, p)
                    End Using
                End Using
            End If
            Dim textLeft As Single = 6 * s
            If HasIconFont AndAlso Glyph.Length > 0 Then
                DrawIcon(g, Glyph, New RectangleF(4 * s, 0, 26 * s, Height), FlyoutTheme.TextPrimary)
                textLeft = 34 * s
            End If
            DrawText(g, Text, New RectangleF(textLeft, 0, Width - textLeft, Height),
                     FlyoutTheme.TextPrimary, StringAlignment.Near)
        End Sub

        Protected Overrides Sub OnMouseEnter(e As EventArgs)
            MyBase.OnMouseEnter(e)
            _hover = True
            Cursor = Cursors.Hand
            InvalidateParent()
        End Sub

        Protected Overrides Sub OnMouseLeave(e As EventArgs)
            MyBase.OnMouseLeave(e)
            _hover = False
            Cursor = Cursors.Default
            InvalidateParent()
        End Sub

    End Class

    Friend NotInheritable Class FlyoutLabel
        Inherits FlyoutControlBase

        Friend Sub New()
            Height = CInt(32 * Sc)
        End Sub

        Protected Overrides Sub PaintContent(g As Graphics)
            DrawText(g, Text, New RectangleF(6 * Sc, 0, Width - 12 * Sc, Height),
                     FlyoutTheme.TextSecondary, StringAlignment.Near)
        End Sub

    End Class

End Namespace
