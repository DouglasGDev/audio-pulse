//! Mirrors the master volume onto the Windows default playback endpoint, so
//! the app slider and the Windows volume controller are the same control.

#[derive(Debug, Clone, PartialEq)]
pub struct DefaultVolume {
    pub id: String,
    pub name: String,
    pub scalar: f32,
    pub muted: bool,
}

impl DefaultVolume {
    /// Mute sits above the scalar: unmuting restores the previous level.
    pub fn effective(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.scalar.clamp(0.0, 1.0)
        }
    }
}

/// The level a user actually heard under the old model, where the DSP gain and
/// the endpoint scalar multiplied. Used once, when the sync is introduced.
pub fn migrated_master(dsp_volume: f32, endpoint: &DefaultVolume) -> f32 {
    (dsp_volume.clamp(0.0, 1.0) * endpoint.effective()).clamp(0.0, 1.0)
}

/// Endpoint values are rounded by some drivers; anything below this is the
/// same volume.
pub const EPSILON: f32 = 0.006;

pub fn differs(a: f32, b: f32) -> bool {
    (a - b).abs() > EPSILON
}

pub fn supported() -> bool {
    cfg!(windows)
}

pub fn read_default() -> Result<DefaultVolume, String> {
    imp::read_default()
}

/// Makes the Windows endpoint the master: a positive value also clears mute,
/// while zero mutes the endpoint and preserves its scalar for later.
pub fn write_default(volume: f32) -> Result<(), String> {
    imp::write_default(volume.clamp(0.0, 1.0))
}

/// Forces an endpoint outside the route path to a fixed scalar. Used to keep
/// the physical output at unity and the virtual capture at unity, so the
/// Windows master is the only gain in the chain.
pub fn normalize_endpoint(name: &str, target: f32) -> Result<(), String> {
    imp::normalize_endpoint(name, target.clamp(0.0, 1.0))
}

#[cfg(windows)]
mod imp {
    use super::DefaultVolume;
    use crate::audio::win_endpoint::Session;

    pub fn read_default() -> Result<DefaultVolume, String> {
        let session = Session::new()?;
        let state = session.default_state()?;
        Ok(DefaultVolume {
            id: state.endpoint.id,
            name: state.endpoint.name,
            scalar: state.volume.scalar,
            muted: state.volume.muted,
        })
    }

    pub fn write_default(volume: f32) -> Result<(), String> {
        let session = Session::new()?;
        if volume <= 0.0005 {
            session.apply_default_volume(None, Some(true))
        } else {
            session.apply_default_volume(Some(volume), Some(false))
        }
    }

    pub fn normalize_endpoint(name: &str, target: f32) -> Result<(), String> {
        let session = Session::new()?;
        let Some(state) = session.state_by_name(name)? else {
            return Err(format!("A saída \"{name}\" não apareceu no Windows."));
        };
        let unchanged = !state.muted && (state.scalar - target).abs() <= 0.001;
        if unchanged {
            return Ok(());
        }
        session.apply_volume_by_name(name, Some(target), Some(false))?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::DefaultVolume;

    pub fn read_default() -> Result<DefaultVolume, String> {
        Err("A sincronização de volume com o sistema está disponível apenas no Windows.".into())
    }

    pub fn write_default(_volume: f32) -> Result<(), String> {
        Ok(())
    }

    pub fn normalize_endpoint(_name: &str, _target: f32) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(scalar: f32, muted: bool) -> DefaultVolume {
        DefaultVolume {
            id: "id".into(),
            name: "name".into(),
            scalar,
            muted,
        }
    }

    #[test]
    fn mute_wins_over_scalar() {
        assert_eq!(endpoint(0.7, true).effective(), 0.0);
        assert_eq!(endpoint(0.7, false).effective(), 0.7);
    }

    #[test]
    fn migration_preserves_the_audible_level() {
        assert_eq!(migrated_master(1.0, &endpoint(0.4, false)), 0.4);
        assert_eq!(migrated_master(0.5, &endpoint(0.5, false)), 0.25);
        assert_eq!(migrated_master(0.8, &endpoint(0.8, true)), 0.0);
    }

    #[test]
    fn endpoint_rounding_is_not_a_change() {
        assert!(!differs(0.4001, 0.4));
        assert!(differs(0.41, 0.4));
    }
}
