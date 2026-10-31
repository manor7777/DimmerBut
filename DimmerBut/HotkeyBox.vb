Imports System.ComponentModel
Imports System.Drawing
Imports System.Windows.Forms

Namespace DimmerBut

    ' Read-only text box that records a key combination (modifier + key).
    ' Press Delete, Backspace or Esc (with no modifier) to clear it.
    Friend NotInheritable Class HotkeyBox
        Inherits TextBox

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property Mods As UInteger

        <Browsable(False), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)>
        Friend Property VKey As Integer

        Friend Sub New()
            Me.ReadOnly = True
            BackColor = SystemColors.Window
            ShortcutsEnabled = False
            ShowText()
        End Sub

        Friend Sub SetHotkey(mods As UInteger, key As Integer)
            Me.Mods = mods
            VKey = key
            ShowText()
        End Sub

        Private Sub ShowText()
            Text = Format(Mods, VKey)
        End Sub

        Friend Shared Function Format(mods As UInteger, key As Integer) As String
            If key = 0 Then Return "None"
            Dim parts As New List(Of String)
            If (mods And NativeMethods.MOD_CONTROL) <> 0 Then parts.Add("CTRL")
            If (mods And NativeMethods.MOD_ALT) <> 0 Then parts.Add("ALT")
            If (mods And NativeMethods.MOD_SHIFT) <> 0 Then parts.Add("SHIFT")
            parts.Add(CType(key, Keys).ToString())
            Return String.Join(" + ", parts)
        End Function

        Protected Overrides Function ProcessCmdKey(ByRef msg As Message, keyData As Keys) As Boolean
            Dim key = keyData And Keys.KeyCode

            Select Case key
                Case Keys.ControlKey, Keys.ShiftKey, Keys.Menu, Keys.LWin, Keys.RWin
                    Return True
            End Select

            Dim m As UInteger = 0
            If (keyData And Keys.Control) <> 0 Then m = m Or NativeMethods.MOD_CONTROL
            If (keyData And Keys.Alt) <> 0 Then m = m Or NativeMethods.MOD_ALT
            If (keyData And Keys.Shift) <> 0 Then m = m Or NativeMethods.MOD_SHIFT

            If m = 0 Then
                If key = Keys.Delete OrElse key = Keys.Back OrElse key = Keys.Escape Then
                    SetHotkey(0UI, 0)
                End If
                Return True
            End If

            SetHotkey(m, CInt(key))
            Return True
        End Function

    End Class

End Namespace