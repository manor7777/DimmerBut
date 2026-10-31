Namespace DimmerBut

    ' f.lux-style schedule: Daytime -> Sunset -> Bedtime -> back to Daytime in the morning.
    ' Sunrise/sunset are calculated offline from the saved latitude/longitude.
    '
    '   morning ease : Bedtime K -> Daytime K   (starts at the later of sunrise and wake time)
    '   daytime      : Daytime K
    '   sunset ease  : Daytime K -> Sunset K    (starts at sunset)
    '   evening      : Sunset K
    '   bed ease     : Sunset K -> Bedtime K    (starts 8 hours before wake time)
    '   night        : Bedtime K
    Friend NotInheritable Class NightMode

        Private Const SleepMinutes As Double = 480.0
        Private Const Zenith As Double = 90.833

        Private Shared _cacheDate As Date = Date.MinValue
        Private Shared _cacheLat As Double
        Private Shared _cacheLon As Double
        Private Shared _cacheRise As Double
        Private Shared _cacheSet As Double

        Friend Shared Function GetKelvin(s As AppSettings, nowLocal As Date) As Double
            Dim rise As Double, sset As Double
            GetSunMinutes(s, nowLocal.Date, rise, sset)

            Dim tr As Double = Math.Max(1, s.TransitionMinutes)
            Dim wake As Double = s.WakeMinutes

            Dim m0 As Double = Math.Clamp(Math.Max(rise, wake), 0.0, 1439.0)
            Dim m1 As Double = m0 + tr
            Dim s0 As Double = Math.Max(sset, m1)
            Dim s1 As Double = s0 + tr

            Dim bed As Double = (((wake - SleepMinutes) Mod 1440.0) + 1440.0) Mod 1440.0
            If bed < m0 Then bed += 1440.0
            Dim endFrame As Double = m0 + 1440.0
            Dim b0 As Double = Math.Min(Math.Max(bed, s1), endFrame - tr)
            Dim b1 As Double = b0 + tr

            Dim t As Double = nowLocal.TimeOfDay.TotalMinutes
            If t < m0 Then t += 1440.0

            Dim dayK As Double = s.DayKelvin
            Dim sunK As Double = s.SunsetKelvin
            Dim bedK As Double = s.BedKelvin

            If t < m1 Then Return Ease(bedK, dayK, (t - m0) / tr)
            If t < s0 Then Return dayK
            If t < s1 Then Return Ease(dayK, sunK, (t - s0) / tr)
            If t < b0 Then Return sunK
            If t < b1 Then Return Ease(sunK, bedK, (t - b0) / tr)
            Return bedK
        End Function

        Private Shared Function Ease(a As Double, b As Double, x As Double) As Double
            Dim u = Math.Clamp(x, 0.0, 1.0)
            u = u * u * (3.0 - 2.0 * u)
            Return a + (b - a) * u
        End Function

        ' Per-channel gains (0..1) for a colour temperature, relative to 6500K = no change.
        Friend Shared Function GetGains(kelvin As Double) As Single()
            Dim c = KelvinToRgb(kelvin)
            Dim w = KelvinToRgb(6500.0)
            Return {
                CSng(Math.Min(1.0, c(0) / w(0))),
                CSng(Math.Min(1.0, c(1) / w(1))),
                CSng(Math.Min(1.0, c(2) / w(2)))
            }
        End Function

        Private Shared Function KelvinToRgb(kelvin As Double) As Double()
            Dim t As Double = Math.Clamp(kelvin, 1000.0, 6500.0) / 100.0
            Dim r As Double, g As Double, b As Double

            If t <= 66.0 Then
                r = 255.0
                g = 99.4708025861 * Math.Log(t) - 161.1195681661
            Else
                r = 329.698727446 * Math.Pow(t - 60.0, -0.1332047592)
                g = 288.1221695283 * Math.Pow(t - 60.0, -0.0755148492)
            End If

            If t >= 66.0 Then
                b = 255.0
            ElseIf t <= 19.0 Then
                b = 0.0
            Else
                b = 138.5177312231 * Math.Log(t - 10.0) - 305.0447927307
            End If

            Return {Math.Clamp(r, 0.0, 255.0) / 255.0,
                    Math.Clamp(g, 0.0, 255.0) / 255.0,
                    Math.Clamp(b, 0.0, 255.0) / 255.0}
        End Function

        ' ---- Sunrise / sunset (NOAA / Almanac algorithm), returned as local minutes after midnight ----
        Private Shared Sub GetSunMinutes(s As AppSettings, d As Date, ByRef rise As Double, ByRef sset As Double)
            If Not s.HasLocation Then
                rise = 7.0 * 60.0
                sset = 19.0 * 60.0
                Return
            End If

            If d = _cacheDate AndAlso s.Latitude = _cacheLat AndAlso s.Longitude = _cacheLon Then
                rise = _cacheRise
                sset = _cacheSet
                Return
            End If

            Dim r = SunMinutes(d, s.Latitude, s.Longitude, True)
            Dim st = SunMinutes(d, s.Latitude, s.Longitude, False)

            If Not r.HasValue OrElse Not st.HasValue OrElse r.Value >= st.Value Then
                rise = 6.0 * 60.0
                sset = 18.0 * 60.0
            Else
                rise = r.Value
                sset = st.Value
            End If

            _cacheDate = d
            _cacheLat = s.Latitude
            _cacheLon = s.Longitude
            _cacheRise = rise
            _cacheSet = sset
        End Sub

        Private Shared Function Rad(x As Double) As Double
            Return x * Math.PI / 180.0
        End Function

        Private Shared Function Deg(x As Double) As Double
            Return x * 180.0 / Math.PI
        End Function

        Private Shared Function Norm(x As Double, m As Double) As Double
            Return ((x Mod m) + m) Mod m
        End Function

        Private Shared Function SunMinutes(d As Date, lat As Double, lon As Double, rising As Boolean) As Double?
            Dim n As Integer = d.DayOfYear
            Dim lngHour As Double = lon / 15.0
            Dim t As Double = n + ((If(rising, 6.0, 18.0)) - lngHour) / 24.0
            Dim m As Double = 0.9856 * t - 3.289

            Dim l As Double = m + 1.916 * Math.Sin(Rad(m)) + 0.02 * Math.Sin(Rad(2.0 * m)) + 282.634
            l = Norm(l, 360.0)

            Dim ra As Double = Deg(Math.Atan(0.91764 * Math.Tan(Rad(l))))
            ra = Norm(ra, 360.0)
            Dim lQuad As Double = Math.Floor(l / 90.0) * 90.0
            Dim raQuad As Double = Math.Floor(ra / 90.0) * 90.0
            ra = (ra + (lQuad - raQuad)) / 15.0

            Dim sinDec As Double = 0.39782 * Math.Sin(Rad(l))
            Dim cosDec As Double = Math.Cos(Math.Asin(sinDec))

            Dim cosH As Double = (Math.Cos(Rad(Zenith)) - sinDec * Math.Sin(Rad(lat))) / (cosDec * Math.Cos(Rad(lat)))
            If cosH > 1.0 OrElse cosH < -1.0 Then Return Nothing

            Dim h As Double
            If rising Then
                h = (360.0 - Deg(Math.Acos(cosH))) / 15.0
            Else
                h = Deg(Math.Acos(cosH)) / 15.0
            End If

            Dim tt As Double = h + ra - 0.06571 * t - 6.622
            Dim ut As Double = Norm(tt - lngHour, 24.0)

            Dim utc As Date = DateTime.SpecifyKind(d.Date, DateTimeKind.Utc).AddHours(ut)
            Dim local As Date = TimeZoneInfo.ConvertTimeFromUtc(utc, TimeZoneInfo.Local)
            Return local.TimeOfDay.TotalMinutes
        End Function

    End Class

End Namespace
