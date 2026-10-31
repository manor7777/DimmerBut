Imports System.Net.Http
Imports System.Text.Json
Imports System.Threading.Tasks

Namespace DimmerBut

    Friend NotInheritable Class GeoResult
        Friend Property DisplayName As String = ""
        Friend Property Latitude As Double
        Friend Property Longitude As Double
    End Class

    ' One-off city lookup using the free Open-Meteo geocoding API (no key, no account).
    ' Only the city name you type is sent; the coordinates are then stored locally and
    ' all sunrise/sunset maths is done offline.
    Friend NotInheritable Class Geocoder

        Private Shared ReadOnly Http As New HttpClient With {.Timeout = TimeSpan.FromSeconds(10)}

        Friend Shared Async Function FindAsync(query As String) As Task(Of GeoResult)
            Dim parts = query.Split({","c}, 2)
            Dim place As String = parts(0).Trim()
            Dim qual As String = If(parts.Length > 1, parts(1).Trim(), "")
            If place.Length < 2 Then Return Nothing

            Dim url As String = "https://geocoding-api.open-meteo.com/v1/search?name=" &
                Uri.EscapeDataString(place) & "&count=20&language=en&format=json"

            Dim json As String = Await Http.GetStringAsync(url)

            Using doc = JsonDocument.Parse(json)
                Dim results As JsonElement
                If Not doc.RootElement.TryGetProperty("results", results) Then Return Nothing

                For Each r In results.EnumerateArray()
                    Dim country = Str(r, "country")
                    Dim admin1 = Str(r, "admin1")
                    Dim admin2 = Str(r, "admin2")
                    Dim cc = Str(r, "country_code")

                    If qual.Length = 0 OrElse Matches(qual, country, admin1, admin2, cc) Then
                        Dim res As New GeoResult()
                        Dim nm = Str(r, "name")
                        Dim bits As New List(Of String) From {nm}
                        If admin1.Length > 0 AndAlso admin1 <> nm Then bits.Add(admin1)
                        If country.Length > 0 Then bits.Add(country)
                        res.DisplayName = String.Join(", ", bits)
                        res.Latitude = r.GetProperty("latitude").GetDouble()
                        res.Longitude = r.GetProperty("longitude").GetDouble()
                        Return res
                    End If
                Next
            End Using

            Return Nothing
        End Function

        Private Shared Function Str(e As JsonElement, name As String) As String
            Dim v As JsonElement
            If e.TryGetProperty(name, v) AndAlso v.ValueKind = JsonValueKind.String Then
                Return If(v.GetString(), "")
            End If
            Return ""
        End Function

        Private Shared Function Matches(qual As String, ParamArray candidates() As String) As Boolean
            Dim q = qual.Trim().Trim("."c).Replace(".", "")
            Select Case q.ToUpperInvariant()
                Case "UK", "GREAT BRITAIN", "BRITAIN", "ENGLAND", "SCOTLAND", "WALES", "NORTHERN IRELAND"
                    If q.ToUpperInvariant() = "UK" OrElse q.ToUpperInvariant().EndsWith("BRITAIN") Then q = "GB"
                Case "USA", "US", "UNITED STATES OF AMERICA"
                    q = "US"
            End Select

            For Each c In candidates
                If c.Length > 0 AndAlso c.Equals(q, StringComparison.OrdinalIgnoreCase) Then Return True
            Next
            Return False
        End Function

    End Class

End Namespace
