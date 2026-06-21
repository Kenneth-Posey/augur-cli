//! Test support: consolidator domain value trait implementations.
//!
//! Provides `DomainValue`, `LayerRank`, and `GraphMetadataTestAccess` traits
//! that expose otherwise private domain fields for testing purposes.

use augur_core::consolidator::domain::{
    AnalysisId, ArchitectureLayer, CallDepth, CallGraphId, CodeVersion, ConfidenceScore, EdgeCount,
    FunctionCount, FunctionId, GraphMetadata, GraphMetadataOptional, GraphNodeCount, InDegree,
    IterationCount, LayerName, LineNumber, LinesSaved, ModulePath, OpportunitiesCount, OutDegree,
    ParseErrorCount, PercentComplete, ReportId, SignatureNorm, TimestampMs,
};

pub trait DomainValue {
    type Value;
    fn value(&self) -> Self::Value;
}

impl DomainValue for ConfidenceScore {
    type Value = f64;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for LinesSaved {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for GraphNodeCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for EdgeCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for CallDepth {
    type Value = u32;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for IterationCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for TimestampMs {
    type Value = u64;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for LineNumber {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for InDegree {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for OutDegree {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for PercentComplete {
    type Value = f64;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for FunctionCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for ParseErrorCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for OpportunitiesCount {
    type Value = usize;
    fn value(&self) -> Self::Value {
        (*self).into()
    }
}

impl DomainValue for FunctionId {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.0.clone()
    }
}

impl DomainValue for ModulePath {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for SignatureNorm {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for LayerName {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for CallGraphId {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for AnalysisId {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for ReportId {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

impl DomainValue for CodeVersion {
    type Value = String;
    fn value(&self) -> Self::Value {
        self.clone().into()
    }
}

pub trait LayerRank {
    fn rank(&self) -> usize;
}

const _: fn(&ArchitectureLayer) -> usize = LayerRank::rank;

impl LayerRank for ArchitectureLayer {
    fn rank(&self) -> usize {
        match self {
            ArchitectureLayer::Domain => 0,
            ArchitectureLayer::Logic => 1,
            ArchitectureLayer::Adapter => 2,
            ArchitectureLayer::Wiring => 3,
            ArchitectureLayer::TuiSurface => 4,
            ArchitectureLayer::Test => 5,
            ArchitectureLayer::External => 6,
        }
    }
}

pub trait GraphMetadataTestAccess {
    fn notes_view(&self) -> Option<&str>;
    fn derivation_path_view(&self) -> Option<&Vec<String>>;
    fn set_derivation_path_view(&mut self, path: Option<Vec<String>>);
}

const _: fn(&GraphMetadata) -> Option<&str> = GraphMetadataTestAccess::notes_view;
const _: fn(&GraphMetadata) -> Option<&Vec<String>> = GraphMetadataTestAccess::derivation_path_view;
const _: fn(&mut GraphMetadata, Option<Vec<String>>) =
    GraphMetadataTestAccess::set_derivation_path_view;

impl GraphMetadataTestAccess for GraphMetadata {
    fn notes_view(&self) -> Option<&str> {
        self.core.notes.as_deref()
    }

    fn derivation_path_view(&self) -> Option<&Vec<String>> {
        self.optional
            .as_ref()
            .and_then(|opt| opt.derivation_path.as_ref())
    }

    fn set_derivation_path_view(&mut self, path: Option<Vec<String>>) {
        if let Some(path) = path {
            if self.optional.is_none() {
                self.optional = Some(GraphMetadataOptional {
                    derivation_path: Some(path),
                    optimization_notes: None,
                });
            } else if let Some(opt) = &mut self.optional {
                opt.derivation_path = Some(path);
            }
        } else if let Some(opt) = &mut self.optional {
            opt.derivation_path = None;
        }
    }
}
