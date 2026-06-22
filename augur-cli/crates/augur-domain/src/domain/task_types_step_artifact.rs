impl StepArtifact {
    /// Build a validated step artifact.
    pub fn new(
        name: impl Into<ArtifactName>,
        data: impl Into<ArtifactData>,
    ) -> Result<Self, ExecutionPlanError> {
        let name: ArtifactName = name.into();
        if name.is_empty() {
            return Err(ExecutionPlanError::EmptyArtifactName);
        }

        Ok(Self {
            name,
            data: data.into(),
        })
    }

    /// Borrow the artifact name as a semantic reference wrapper.
    pub fn name(&self) -> ArtifactNameRef<'_> {
        ArtifactNameRef(self.name.as_str())
    }

    /// Borrow the artifact payload as a semantic reference wrapper.
    pub fn data(&self) -> ArtifactDataRef<'_> {
        ArtifactDataRef(self.data.as_str())
    }
}

#[derive(serde::Deserialize)]
struct RawStepArtifact {
    name: ArtifactName,
    data: ArtifactData,
}

impl TryFrom<RawStepArtifact> for StepArtifact {
    type Error = String;

    fn try_from(value: RawStepArtifact) -> Result<Self, Self::Error> {
        if value.name.is_empty() {
            return Err("step artifact name must not be empty".to_string());
        }
        Ok(Self {
            name: value.name,
            data: value.data,
        })
    }
}