Imports System.Threading
Imports System.Windows.Forms

Namespace DimmerBut

    Friend NotInheritable Class Program

        <STAThread>
        Friend Shared Sub Main()
            Dim createdNew As Boolean
            Using mtx As New Mutex(True, "DimmerBut.SingleInstance", createdNew)
                If Not createdNew Then Return

                Application.EnableVisualStyles()
                Application.SetCompatibleTextRenderingDefault(False)
                Application.SetHighDpiMode(HighDpiMode.PerMonitorV2)

                Using ctx As New TrayContext()
                    Application.Run(ctx)
                End Using
            End Using
        End Sub

    End Class

End Namespace
