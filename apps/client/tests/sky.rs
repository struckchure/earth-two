//! The sky's tests, ported from game/sky_test.go and game/weather_test.go,
//! with the clock, the daylight and the effects checked the way the Go
//! code says they behave (they had no tests of their own).

use bevy::prelude::*;
use earth_two_client::sky::{
    self, Ambient, Clock, Daylight, Effects, Haze, SunLight, UnixTime, Weather, conditions,
    daylight::Sun,
    effects::MAX_PARTICLES,
    skytex::{SKY_TEX_H, SKY_TEX_W, paint_sky, pixel_dir, sky_at},
    stars::{
        STAR_BAND, STAR_CLASSES, STAR_COUNT, galaxy_core, galaxy_pole, milky_way, stars,
        visible_stars,
    },
    storm_at,
    sun::{
        AU_KM, EARTH_KM, ECLIPTIC_TILT, NEIGHBOURS, PLANET_SCALE, THE_RED, dusk_at, ecliptic,
        glare, light_from, neighbour_at, night_at, sun_at, sun_radius,
    },
    time::MINUTE,
    weather::blow_to,
};
use std::{collections::HashSet, f64::consts::PI};

fn day() -> UnixTime {
    UnixTime::utc(2026, 10, 6, 0, 0, 0)
}

fn up_at(h: f64) -> f64 {
    f64::from(sun_at(day().add_hours(h)).y).asin() * 180.0 / PI
}

// TestTheSunIsTRAPPIST1FromTheRed
#[test]
fn the_sun_is_trappist1_from_the_red() {
    // 0.1192 solar radii seen from 0.02925 AU: 2.2° across.
    let d = 2.0 * sun_radius() * 180.0 / PI;
    assert!(
        (2.1..=2.3).contains(&d),
        "the sun is {d:.2}° across, want about 2.2°"
    );
}

// TestTheSunRisesAndSets
#[test]
fn the_sun_rises_and_sets() {
    // Over a day: up from about 05:00 to 19:00, low even at noon, down in
    // the night; dusk's colour at the ends of the day, dark at midnight.
    let at = |h: f64| sun_at(day().add_hours(h));
    for h in [6.0, 9.0, 12.0, 15.0, 18.0] {
        let up = up_at(h);
        assert!(up > 0.0, "at {h}:00 the sun's down ({up:.1}°), want it up");
    }
    for h in [0.0, 2.0, 22.0] {
        let up = up_at(h);
        assert!(up < 0.0, "at {h}:00 the sun's up ({up:.1}°), want night");
    }
    let noon = up_at(12.0);
    assert!(noon <= 32.0, "at noon the sun's {noon:.1}° up, want it low");
    assert!(
        dusk_at(at(12.0)) <= 0.0,
        "at noon, {} of dusk's colour",
        dusk_at(at(12.0))
    );
    assert!(
        dusk_at(at(18.7)) >= 0.5,
        "at 18:42, only {} of dusk's colour",
        dusk_at(at(18.7))
    );
    assert!(
        night_at(at(0.0)) >= 1.0,
        "at midnight, only {} of the way into night",
        night_at(at(0.0))
    );
    assert!(
        night_at(at(12.0)) <= 0.0,
        "at noon, {} of the way into night",
        night_at(at(12.0))
    );
}

// TestNeighboursAreOnTheEcliptic
#[test]
fn neighbours_are_on_the_ecliptic() {
    let sun = sun_at(UnixTime::now());
    let (s, v) = ecliptic(sun);
    let normal = s.cross(v);
    for n in &NEIGHBOURS {
        for days in [0.0, 3.3, 11.7] {
            let (dir, _) = neighbour_at(n, sun, days);
            let off = dir.dot(normal);
            assert!(
                off.abs() <= 1e-4,
                "{} on day {days} is {off} off the ecliptic",
                n.name
            );
        }
    }
}

// TestTheEclipticStaysLow
#[test]
fn the_ecliptic_stays_low() {
    let (s, v) = ecliptic(sun_at(UnixTime::now()));
    let d = s.dot(v);
    assert!(d.abs() <= 1e-5, "v isn't a quarter round from the sun: {d}");
    let mut high: f64 = 0.0;
    let mut a = 0.0;
    while a < 2.0 * PI {
        let dir = s * a.cos() as f32 + v * a.sin() as f32;
        high = high.max(f64::from(dir.y).asin() * 180.0 / PI);
        a += 0.01;
    }
    let want = ECLIPTIC_TILT * 180.0 / PI;
    assert!(
        (high - want).abs() <= 0.5,
        "the ecliptic rises to {high:.1}°, want {want:.1}°"
    );
}

// TestNeighboursSizes
#[test]
fn neighbours_sizes() {
    let sun = sun_at(UnixTime::now());
    for n in &NEIGHBOURS {
        // Nearest, at conjunction on the same side of the sun, and
        // furthest, on the far side; drawn PLANET_SCALE times their size.
        let near = (n.a - THE_RED.a).abs() * AU_KM;
        let far = (n.a + THE_RED.a) * AU_KM;
        let lo = PLANET_SCALE * (n.r * EARTH_KM / far).asin();
        let hi = PLANET_SCALE * (n.r * EARTH_KM / near).asin();
        let mut days = 0.0;
        while days < 40.0 {
            let (_, r) = neighbour_at(n, sun, days);
            assert!(
                r >= lo - 1e-9 && r <= hi + 1e-9,
                "{} on day {days}: radius {r} outside {lo}..{hi}",
                n.name
            );
            days += 0.5;
        }
    }
    // f at its nearest is about the Moon's size (0.55° across), as drawn
    // PLANET_SCALE times bigger.
    let f = NEIGHBOURS[3];
    let d = 2.0 * (f.r * EARTH_KM / ((f.a - THE_RED.a).abs() * AU_KM)).asin() * 180.0 / PI;
    assert!(
        (0.5..=0.6).contains(&d),
        "f at its nearest is {d:.2}° across, want about 0.55°"
    );
}

// TestNeighboursMove
#[test]
fn neighbours_move() {
    let sun = sun_at(UnixTime::now());
    for n in &NEIGHBOURS {
        let (a, _) = neighbour_at(n, sun, 0.0);
        let (b, _) = neighbour_at(n, sun, 1.0);
        // They come round again in their synodic period, so a day moves
        // them 360°/that.
        let synodic = 1.0 / (1.0 / n.p - 1.0 / THE_RED.p).abs();
        let moved = f64::from(a.dot(b).min(1.0)).acos() * 180.0 / PI;
        assert!(
            moved >= 0.1 && moved <= 360.0 / synodic * 3.0,
            "{} moved {moved:.2}° in a day, synodic period {synodic:.1} days",
            n.name
        );
    }
}

// TestGlareThinsOut
#[test]
fn glare_thins_out() {
    let mut last = 2.0f32;
    let mut deg = 0.0;
    while deg <= 90.0 {
        let g = glare(deg * PI / 180.0);
        assert!(
            g <= last + 1e-6,
            "glare grows again {deg}° out: {g} after {last}"
        );
        last = g;
        deg += 0.5;
    }
    assert!(
        glare(0.0) >= 0.9,
        "glare at the sun {}, want it dense",
        glare(0.0)
    );
    let quarter = glare(90.0 * PI / 180.0);
    assert!(
        quarter <= 0.1,
        "glare a quarter of the sky away {quarter}, want it all but gone"
    );
}

// TestTheStarsAreMostlyFaint
#[test]
fn the_stars_are_mostly_faint() {
    // Brightest first; most faint, a few bright; every colour there.
    let stars = stars();
    assert_eq!(stars.len(), STAR_COUNT + STAR_BAND);
    let (mut bright, mut colours) = (0, HashSet::new());
    for (i, s) in stars.iter().enumerate() {
        assert!(
            i == 0 || s.bright <= stars[i - 1].bright,
            "star {i} is brighter than the one before it"
        );
        if s.bright > 0.75 {
            bright += 1;
        }
        colours.insert(s.colour);
    }
    assert!(
        bright >= 10 && bright <= stars.len() / 20,
        "{bright} bright stars of {}, want a few",
        stars.len()
    );
    assert_eq!(
        colours.len(),
        STAR_CLASSES.len(),
        "star colours, want every class"
    );
    let median = stars[stars.len() / 2].bright;
    assert!(
        median <= 0.35,
        "the median star is {median} bright, want most faint"
    );
}

// TestTheMilkyWayIsABand
#[test]
fn the_milky_way_is_a_band() {
    // Bright along its plane, nothing at its poles.
    let core = milky_way(galaxy_core());
    assert!(core >= 0.2, "at its core the Milky Way is {core} bright");
    let pole = milky_way(galaxy_pole());
    assert!(
        pole <= 0.01,
        "at its pole the Milky Way is {pole} bright, want none"
    );
}

// TestTheWeatherIsMostlyGood
#[test]
fn the_weather_is_mostly_good() {
    // Over two months, minute by minute: mostly clear, now and then
    // dusty, once in a while a storm, and never out of 0 to 1.
    let start = UnixTime::utc(2026, 10, 1, 0, 0, 0);
    let end = UnixTime::utc(2026, 12, 1, 0, 0, 0);
    let (mut steps, mut dusty, mut stormy) = (0, 0, 0);
    let mut storm_days = HashSet::new();
    let mut at = start;
    while at < end {
        let s = storm_at(at);
        assert!(
            (0.0..=1.0).contains(&s),
            "at {at:?} the weather's {s}, want 0 to 1"
        );
        match conditions(s) {
            "Dusty" => dusty += 1,
            "Dust storm" => {
                stormy += 1;
                storm_days.insert(at.year_day());
            }
            _ => {}
        }
        steps += 1;
        at = at.after(MINUTE);
    }
    let clear = steps - dusty - stormy;
    let share = |n: i32| n as f32 / steps as f32;
    println!(
        "clear {:.1}%, dusty {:.1}%, storm {:.1}% of the time; storms on {} days of 61",
        100.0 * share(clear),
        100.0 * share(dusty),
        100.0 * share(stormy),
        storm_days.len()
    );
    assert!(
        share(clear) >= 0.75,
        "clear {:.0}% of the time, want most of it",
        100.0 * share(clear)
    );
    assert!(
        share(dusty) >= 0.03 && share(dusty) <= 0.2,
        "dusty {:.1}% of the time, want now and then",
        100.0 * share(dusty)
    );
    assert!(
        share(stormy) > 0.0 && share(stormy) <= 0.05,
        "a dust storm {:.1}% of the time, want once in a while",
        100.0 * share(stormy)
    );
    let n = storm_days.len();
    assert!(
        (5..=40).contains(&n),
        "dust storms on {n} days of 61, want once in a while"
    );
}

// TestStormsBlowUpSlowly
#[test]
fn storms_blow_up_slowly() {
    // Minute to minute a storm changes by no more than a rise allows.
    let start = UnixTime::utc(2026, 10, 6, 0, 0, 0);
    let end = start.add_hours(72.0);
    let mut last = storm_at(start);
    let mut at = start;
    while at < end {
        let s = storm_at(at);
        let d = s - last;
        assert!(
            d.abs() <= 0.4,
            "at {at:?} the storm jumped from {last} to {s} in a minute"
        );
        last = s;
        at = at.after(MINUTE);
    }
}

// TestEveryoneSeesTheSameStorm: the clock's a moment since 1970, with no
// zone to differ by; the same moment written two ways is the same.
#[test]
fn everyone_sees_the_same_storm() {
    let at = UnixTime::utc(2026, 10, 6, 14, 30, 0);
    let landfall = UnixTime::utc(2026, 10, 6, 19, 30, 0).after(-5 * sky::time::HOUR);
    assert_eq!(at, landfall);
    assert_eq!(storm_at(at), storm_at(landfall));
}

// TestAStormCanBeHeld
#[test]
fn a_storm_can_be_held() {
    let w = Weather::from_setting(Some("0.8"));
    assert!(
        w.forced == 0.8 && w.storm == 0.8,
        "held storm {} (blowing {}), want 0.8",
        w.forced,
        w.storm
    );
    let w = Weather::from_setting(Some(""));
    assert!(w.forced < 0.0, "with none held, held {}", w.forced);
    let w = Weather::from_setting(None);
    assert!(w.forced < 0.0, "with none held, held {}", w.forced);
}

// The clock, as daylight.go's newDaylight and now have it: EARTH_TWO_HOUR
// holds the sun at that hour today, else it follows the real clock.
#[test]
fn the_sun_can_be_held_at_an_hour() {
    let real = UnixTime::utc(2026, 10, 6, 14, 30, 0);
    assert_eq!(Clock::from_setting(None).at(real), real);
    assert_eq!(Clock::from_setting(Some("x")).at(real), real);
    assert_eq!(
        Clock::from_setting(Some("11")).at(real),
        UnixTime::utc(2026, 10, 6, 11, 0, 0)
    );
    assert_eq!(
        Clock::from_setting(Some("22.5")).at(real),
        UnixTime::utc(2026, 10, 6, 22, 30, 0)
    );
    assert_eq!(Clock::from_setting(Some("25")).hour, 1.0, "wraps past 24");
    assert_eq!(
        Clock::from_setting(Some("-3")).hour,
        0.0,
        "no earlier than midnight"
    );
    let noon = Daylight::at(Clock::held(11.0).at(real));
    assert!(noon.night == 0.0 && noon.dusk == 0.0 && noon.sun.y > 0.0);
    assert!(!noon.lamps_on());
    let night = Daylight::at(Clock::held(22.0).at(real));
    assert!(night.night == 1.0 && night.sun.y < 0.0);
    assert!(night.lamps_on());
    assert_eq!(sky::part_of_day(noon.sun, 11.0), "Day");
    assert_eq!(sky::part_of_day(night.sun, 22.0), "Night");
    assert_eq!(
        sky::part_of_day(sun_at(real.day_start().add_hours(5.5)), 5.5),
        "Dawn"
    );
    assert_eq!(
        sky::part_of_day(sun_at(real.day_start().add_hours(18.5)), 18.5),
        "Dusk"
    );
}

// lightFrom: the sun's way by day, never under the ground, and from above
// where the sun went by night.
#[test]
fn the_light_never_comes_from_under_the_ground() {
    let mut h = 0.0;
    while h < 24.0 {
        let sun = sun_at(day().add_hours(h));
        let light = light_from(sun, night_at(sun));
        let up = f64::from(light.y).asin() * 180.0 / PI;
        assert!(up >= 3.0 - 1e-4, "at {h}:00 the light comes from {up:.1}°");
        if night_at(sun) >= 1.0 {
            assert!(
                (up - 25.0).abs() < 1e-3,
                "by night the light is {up:.1}° up, want 25°"
            );
        }
        let flat = Vec3::new(sun.x, 0.0, sun.z).normalize();
        assert!(Vec3::new(light.x, 0.0, light.z).normalize().dot(flat) > 0.999);
        h += 0.25;
    }
}

// The sky's texture: by day pale ochre at the horizon and darker overhead,
// the glare dense at the sun; by night near black, with the Milky Way's
// light somewhere in it; and painted whole, pixel for pixel as sky_at.
#[test]
fn the_sky_is_painted_by_the_hour() {
    let noon = Daylight::at(day().add_hours(11.0));
    let pixels = paint_sky(noon.sun, noon.dusk, noon.night, noon.turn);
    assert_eq!(pixels.len(), SKY_TEX_W * SKY_TEX_H * 4);
    let at = |x: usize, y: usize| {
        let i = (y * SKY_TEX_W + x) * 4;
        sky::rgb(pixels[i], pixels[i + 1], pixels[i + 2])
    };
    let zenith = at(0, 0);
    let horizon = at(0, SKY_TEX_H / 2 - 1);
    assert!(
        horizon.r > zenith.r && horizon.g > zenith.g,
        "horizon {horizon:?} paler than overhead {zenith:?}"
    );
    assert_eq!(
        at(100, 100),
        sky_at(
            pixel_dir(100, 100, SKY_TEX_W, SKY_TEX_H),
            noon.sun,
            noon.dusk,
            noon.night,
            noon.turn
        )
    );
    let at_sun = sky_at(noon.sun, noon.sun, noon.dusk, noon.night, noon.turn);
    assert!(
        at_sun.r >= 250 && at_sun.g >= 230,
        "no glare at the sun: {at_sun:?}"
    );

    let midnight = Daylight::at(day().add_hours(0.0));
    let dark = paint_sky(midnight.sun, midnight.dusk, midnight.night, midnight.turn);
    let mut brightest = 0u8;
    let mut sum = 0u64;
    for px in dark.chunks(4).take(SKY_TEX_W * SKY_TEX_H / 2) {
        brightest = brightest.max(px[0]).max(px[1]).max(px[2]);
        sum += u64::from(px[0]) + u64::from(px[1]) + u64::from(px[2]);
    }
    let mean = sum as f64 / (SKY_TEX_W * SKY_TEX_H / 2 * 3) as f64;
    assert!(mean < 40.0, "the night sky's {mean:.1} bright on average");
    assert!(
        brightest > 60,
        "no Milky Way: the brightest channel is {brightest}"
    );
}

// The stars show only by night, within the budget, and above the horizon.
#[test]
fn the_stars_come_out_at_night() {
    assert!(visible_stars(0.0, 1.0, 0.0, 0.0, usize::MAX).is_empty());
    assert!(
        visible_stars(1.0, 0.0, 0.0, 0.0, usize::MAX).is_empty(),
        "hidden by dust"
    );
    let out = visible_stars(1.0, 1.0, 1.0, 3.0, usize::MAX);
    assert!(
        out.len() > 3000 && out.len() < STAR_COUNT + STAR_BAND,
        "{} stars drawn",
        out.len()
    );
    assert!(out.iter().all(|s| s.dir.y >= -0.02 && s.colour.a >= 7));
    assert!(visible_stars(1.0, 1.0, 1.0, 3.0, 2500).len() <= 2500);
    let clear = visible_stars(1.0, 1.0, 1.0, 3.0, usize::MAX).len();
    let dusty = visible_stars(1.0, 0.3, 1.0, 3.0, usize::MAX).len();
    assert!(dusty < clear, "dust hides none: {dusty} of {clear}");
}

// The weather on the light: by day a storm turns the haze brown and close,
// the sun dim; by night dark; at noon in clear air nothing's changed.
#[test]
fn storms_close_the_haze_in() {
    let noon = Daylight::at(day().add_hours(11.0));
    let (mut w, mut haze, mut ambient, mut sun) = (
        Weather::default(),
        Haze::default(),
        Ambient::default(),
        SunLight::default(),
    );
    blow_to(
        &mut w,
        &noon,
        &mut haze,
        &mut ambient,
        &mut sun,
        0.0,
        1.0 / 60.0,
    );
    assert_eq!(haze, Haze::default());
    assert_eq!(sun, SunLight::default());
    assert_eq!(ambient, Ambient::default());
    for _ in 0..600 {
        blow_to(
            &mut w,
            &noon,
            &mut haze,
            &mut ambient,
            &mut sun,
            1.0,
            1.0 / 60.0,
        );
    }
    assert!(
        w.storm > 0.99,
        "a held storm blows up over a second or two: {}",
        w.storm
    );
    assert!(
        (haze.distance - 70.0).abs() < 0.5,
        "the haze reaches {}",
        haze.distance
    );
    assert!((haze.veil - 0.97).abs() < 0.01);
    assert_eq!(haze.color, sky::weather::STORM_DUST);
    assert!(sun.brightness < 0.3 && sun.color == sky::weather::STORM_SUN);
    assert!(ambient.brightness > Ambient::default().brightness);
    assert_eq!(conditions(w.storm), "Dust storm");

    let midnight = Daylight::at(day().add_hours(0.0));
    let (mut w, mut haze, mut ambient, mut sun) = (
        Weather::default(),
        Haze::default(),
        Ambient::default(),
        SunLight::default(),
    );
    blow_to(
        &mut w,
        &midnight,
        &mut haze,
        &mut ambient,
        &mut sun,
        0.0,
        1.0 / 60.0,
    );
    assert_eq!(haze.color, sky::sun::NIGHT_HORIZON);
    assert_eq!(sun.color, sky::sun::PLANET_LIGHT);
    // By night dusk's colour is all there too (the sun's well past its
    // ramp), so the sun's dimmed by both.
    assert!((sun.brightness - 1.3 * 0.6 * 0.25).abs() < 1e-5);
}

// The particles: emitted, moved on the wind, aged out, and never more than
// MAX_PARTICLES, the oldest giving way.
#[test]
fn the_dust_settles() {
    let mut fx = Effects::seeded(7);
    let light = sky::sun::SUNLIGHT;
    fx.puff(Vec3::ZERO, Vec3::X, 4.0, light);
    assert_eq!(fx.ps.len(), 4);
    fx.ring(Vec3::ZERO, 12, 1.0, light);
    fx.wheel(Vec3::ZERO, Vec3::X * 10.0, 0.5, light);
    fx.sparks(Vec3::Y, Vec3::Y, 5);
    assert_eq!(fx.ps.len(), 22);
    assert!(
        fx.ps
            .iter()
            .all(|p| (p.floor - (p.pos.y - 0.05)).abs() < 1e-6)
    );
    fx.wind = Vec3::new(-1.0, 0.0, 0.0);
    let before: f32 = fx.ps.iter().map(|p| p.pos.x).sum();
    for _ in 0..30 {
        fx.step(1.0 / 60.0);
    }
    let after: f32 = fx.ps.iter().map(|p| p.pos.x).sum();
    assert!(after < before, "the wind carries the dust along -X");
    for _ in 0..600 {
        fx.step(1.0 / 60.0);
    }
    assert!(
        fx.ps.is_empty(),
        "{} particles live past their life",
        fx.ps.len()
    );

    for _ in 0..MAX_PARTICLES + 100 {
        fx.wheel(Vec3::ZERO, Vec3::ZERO, 1.0, light);
    }
    assert_eq!(fx.ps.len(), MAX_PARTICLES);

    // Drawn far to near, behind the camera skipped, close ones thinned.
    let mut fx = Effects::seeded(3);
    for z in [-2.0, -10.0, -0.5, 3.0] {
        fx.add(sky::Particle {
            pos: Vec3::new(0.0, 1.0, z),
            life: 1.0,
            age: 0.3,
            size0: 0.2,
            size1: 0.2,
            colour: sky::rgb(200, 200, 200).with_alpha(255),
            ..Default::default()
        });
    }
    let mut verts = vec![[0.0; 3]; 4 * MAX_PARTICLES];
    let mut cols = vec![sky::Rgba::default(); 4 * MAX_PARTICLES];
    let n = fx.quads(
        Vec3::ZERO,
        -Vec3::Z,
        Vec3::X,
        Vec3::Y,
        &mut verts,
        &mut cols,
    );
    assert_eq!(n, 3);
    assert!(
        verts[0][2] < verts[4][2] && verts[4][2] < verts[8][2],
        "far to near"
    );
    assert!(
        cols[8].a < cols[4].a,
        "the nearest is thinned: {} vs {}",
        cols[8].a,
        cols[4].a
    );
}

// The plugin runs headless: the sun's light turns with the clock, and the
// weather follows the hour.
#[test]
fn the_plugin_runs_headless() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(sky::SkyPlugin)
        .insert_resource(Clock::held(11.0));
    app.update();
    app.update();
    let day = app.world().resource::<Daylight>();
    assert!(day.night == 0.0 && day.sun.y > 0.0);
    let light = day.light;
    let tr = app
        .world_mut()
        .query_filtered::<&Transform, With<Sun>>()
        .single(app.world())
        .expect("one sun");
    assert!(
        tr.forward().dot(-light) > 0.999,
        "the sun's light shines along -light"
    );
    assert!(app.world().resource::<Haze>().veil == 0.0);
    app.world_mut().resource_mut::<Clock>().hour = 0.0;
    app.update();
    let day = app.world().resource::<Daylight>();
    assert!(day.night == 1.0);
    assert_eq!(
        app.world().resource::<SunLight>().color,
        sky::sun::PLANET_LIGHT
    );
}
