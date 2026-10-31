Imports System.Drawing
Imports System.Runtime.InteropServices
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class DarkTheme

        Friend Shared ReadOnly Background As Color = Color.FromArgb(255, 20, 20, 24)
        Friend Shared ReadOnly CardBackground As Color = Color.FromArgb(255, 30, 30, 36)
        Friend Shared ReadOnly SurfaceBackground As Color = Color.FromArgb(255, 42, 42, 50)
        Friend Shared ReadOnly InputBackground As Color = Color.FromArgb(255, 14, 14, 18)
        Friend Shared ReadOnly BorderColor As Color = Color.FromArgb(255, 50, 50, 60)
        Friend Shared ReadOnly Accent As Color = Color.FromArgb(255, 96, 205, 255)
        Friend Shared ReadOnly TextPrimary As Color = Color.FromArgb(255, 245, 245, 248)
        Friend Shared ReadOnly TextSecondary As Color = Color.FromArgb(255, 160, 160, 172)

        <DllImport("uxtheme.dll", ExactSpelling:=True, CharSet:=CharSet.Unicode)>
        Private Shared Function SetWindowTheme(hwnd As IntPtr, pszSubAppName As String, pszSubIdList As String) As Integer
        End Function

        Friend Shared Sub ApplyWindowDarkMode(hwnd As IntPtr)
            Dim dark As Integer = 1
            NativeMethods.DwmSetWindowAttribute(hwnd, 20, dark, 4)   ' DWMWA_USE_IMMERSIVE_DARK_MODE
            Dim corner As Integer = 2
            NativeMethods.DwmSetWindowAttribute(hwnd, 33, corner, 4) ' DWMWA_WINDOW_CORNER_PREFERENCE = Round
        End Sub

        Friend Shared Sub ApplyExplorerTheme(c As Control)
            If c.IsHandleCreated Then
                SetWindowTheme(c.Handle, "Explorer", Nothing)
            Else
                AddHandler c.HandleCreated, Sub() SetWindowTheme(c.Handle, "Explorer", Nothing)
            End If
        End Sub

        Friend Shared Sub StyleAll(parent As Control)
            parent.BackColor = Background
            parent.ForeColor = TextPrimary

            For Each c As Control In parent.Controls
                StyleControl(c)
            Next
        End Sub

        Private Shared Sub StyleControl(c As Control)
            If TypeOf c Is Button Then
                Dim btn = DirectCast(c, Button)
                btn.FlatStyle = FlatStyle.Flat
                btn.BackColor = SurfaceBackground
                btn.ForeColor = TextPrimary
                btn.FlatAppearance.BorderColor = BorderColor
                btn.FlatAppearance.BorderSize = 1
                btn.FlatAppearance.MouseOverBackColor = Color.FromArgb(255, 55, 55, 68)
            ElseIf TypeOf c Is TextBox OrElse TypeOf c Is NumericUpDown OrElse TypeOf c Is DateTimePicker Then
                c.BackColor = InputBackground
                c.ForeColor = TextPrimary
                ApplyExplorerTheme(c)
            ElseIf TypeOf c Is CheckBox Then
                Dim chk = DirectCast(c, CheckBox)
                chk.ForeColor = TextPrimary
                chk.BackColor = Color.Transparent
            ElseIf TypeOf c Is GroupBox Then
                Dim gb = DirectCast(c, GroupBox)
                gb.ForeColor = TextPrimary
                gb.BackColor = Color.Transparent
                For Each child As Control In gb.Controls
                    StyleControl(child)
                Next
            ElseIf TypeOf c Is Panel Then
                c.BackColor = Background
                c.ForeColor = TextPrimary
                For Each child As Control In c.Controls
                    StyleControl(child)
                Next
            ElseIf TypeOf c Is Label Then
                If c.ForeColor <> SystemColors.GrayText AndAlso c.ForeColor <> TextSecondary Then
                    c.ForeColor = TextPrimary
                Else
                    c.ForeColor = TextSecondary
                End If
            End If
        End Sub

    End Class

    ' Dark-themed renderer for ContextMenuStrip
    Friend NotInheritable Class DarkMenuRenderer
        Inherits ToolStripProfessionalRenderer

        Friend Sub New()
            MyBase.New(New DarkColorTable())
        End Sub

        Protected Overrides Sub OnRenderItemText(e As ToolStripItemTextRenderEventArgs)
            e.TextColor = If(e.Item.Enabled, DarkTheme.TextPrimary, DarkTheme.TextSecondary)
            MyBase.OnRenderItemText(e)
        End Sub

        Protected Overrides Sub OnRenderArrow(e As ToolStripArrowRenderEventArgs)
            e.ArrowColor = DarkTheme.TextSecondary
            MyBase.OnRenderArrow(e)
        End Sub

        Protected Overrides Sub OnRenderItemCheck(e As ToolStripItemImageRenderEventArgs)
            Dim g = e.Graphics
            Dim r = e.ImageRectangle
            Using b As New SolidBrush(DarkTheme.Accent)
                g.FillEllipse(b, r.X + 2, r.Y + 2, r.Width - 4, r.Height - 4)
            End Using
        End Sub

    End Class

    Friend NotInheritable Class DarkColorTable
        Inherits ProfessionalColorTable

        Public Overrides ReadOnly Property ToolStripDropDownBackground As Color
            Get
                Return DarkTheme.Background
            End Get
        End Property

        Public Overrides ReadOnly Property ImageMarginGradientBegin As Color
            Get
                Return DarkTheme.Background
            End Get
        End Property

        Public Overrides ReadOnly Property ImageMarginGradientMiddle As Color
            Get
                Return DarkTheme.Background
            End Get
        End Property

        Public Overrides ReadOnly Property ImageMarginGradientEnd As Color
            Get
                Return DarkTheme.Background
            End Get
        End Property

        Public Overrides ReadOnly Property MenuBorder As Color
            Get
                Return DarkTheme.BorderColor
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemBorder As Color
            Get
                Return Color.Transparent
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemSelected As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemSelectedGradientBegin As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemSelectedGradientEnd As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemPressedGradientBegin As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property MenuItemPressedGradientEnd As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property SeparatorDark As Color
            Get
                Return DarkTheme.BorderColor
            End Get
        End Property

        Public Overrides ReadOnly Property SeparatorLight As Color
            Get
                Return Color.Transparent
            End Get
        End Property

        Public Overrides ReadOnly Property CheckBackground As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property CheckSelectedBackground As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

        Public Overrides ReadOnly Property CheckPressedBackground As Color
            Get
                Return DarkTheme.SurfaceBackground
            End Get
        End Property

    End Class

End Namespace
