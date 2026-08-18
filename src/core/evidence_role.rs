use serde::{Deserialize, Serialize};

use crate::core::code_evidence::SourceRole;
use crate::core::research::{ResearchSourceClass, ResearchSourceType};
use crate::core::security::SecuritySourceTier;
use crate::core::source_card::SourceKind;

#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    #[default]
    UnknownOrWeakContext,
    PrimaryImplementation,
    InterfaceOrApiDefinition,
    UsageExample,
    TestOrBehavioralSpecification,
    ConfigurationOrFeatureGate,
    ManifestOrDependencyMetadata,
    OfficialDocumentation,
    ArchitectureOrDesignDocument,
    ReleaseNoteOrChangelog,
    MigrationGuidance,
    BenchmarkOrPerformanceEvidence,
    IssueOrIncidentDiscussion,
    PullRequestOrDesignReview,
    AuthoritativeSecurityAdvisory,
    VendorSecurityGuidance,
    IndependentCorroboration,
    CounterpointOrConflictingEvidence,
    CommunityDiscussion,
}

#[allow(missing_docs)]
impl EvidenceRole {
    pub fn from_source_kind(sk: SourceKind) -> Self {
        match sk {
            SourceKind::Unknown => Self::UnknownOrWeakContext,
            SourceKind::OfficialDocs => Self::OfficialDocumentation,
            SourceKind::PackageRegistry => Self::ManifestOrDependencyMetadata,
            SourceKind::SourceRepository
            | SourceKind::RepositoryRoot
            | SourceKind::SourceDirectory
            | SourceKind::SourceFile
            | SourceKind::Commit => Self::PrimaryImplementation,
            SourceKind::IssueThread => Self::IssueOrIncidentDiscussion,
            SourceKind::PullRequest => Self::PullRequestOrDesignReview,
            SourceKind::ReleaseNotes | SourceKind::Tag => Self::ReleaseNoteOrChangelog,
            SourceKind::SecurityAdvisory => Self::AuthoritativeSecurityAdvisory,
            SourceKind::Reference => Self::InterfaceOrApiDefinition,
            SourceKind::News | SourceKind::Forum => Self::CommunityDiscussion,
            SourceKind::Tutorial => Self::OfficialDocumentation,
        }
    }

    pub fn from_source_role(sr: SourceRole) -> Self {
        match sr {
            SourceRole::Implementation | SourceRole::Generated => Self::PrimaryImplementation,
            SourceRole::Test => Self::TestOrBehavioralSpecification,
            SourceRole::Example => Self::UsageExample,
            SourceRole::Benchmark => Self::BenchmarkOrPerformanceEvidence,
            SourceRole::Configuration | SourceRole::Build | SourceRole::Ci => {
                Self::ConfigurationOrFeatureGate
            }
            SourceRole::Documentation | SourceRole::Readme => Self::OfficialDocumentation,
            SourceRole::Changelog => Self::ReleaseNoteOrChangelog,
            SourceRole::Migration => Self::MigrationGuidance,
            SourceRole::Manifest | SourceRole::Lockfile => Self::ManifestOrDependencyMetadata,
            SourceRole::SecurityPolicy => Self::AuthoritativeSecurityAdvisory,
            SourceRole::Vendor | SourceRole::Unknown => Self::UnknownOrWeakContext,
        }
    }

    pub fn from_research_source_class(rsc: ResearchSourceClass) -> Self {
        match rsc {
            ResearchSourceClass::OfficialDocs | ResearchSourceClass::ReferenceDocs => {
                Self::OfficialDocumentation
            }
            ResearchSourceClass::RepositorySource => Self::PrimaryImplementation,
            ResearchSourceClass::MaintainerIssue => Self::IssueOrIncidentDiscussion,
            ResearchSourceClass::ReleaseNotes => Self::ReleaseNoteOrChangelog,
            ResearchSourceClass::Benchmark => Self::BenchmarkOrPerformanceEvidence,
            ResearchSourceClass::Paper => Self::IndependentCorroboration,
            ResearchSourceClass::StandardSpec => Self::InterfaceOrApiDefinition,
            ResearchSourceClass::SecurityAdvisory => Self::AuthoritativeSecurityAdvisory,
            ResearchSourceClass::VendorBlog => Self::VendorSecurityGuidance,
            ResearchSourceClass::EngineeringBlog
            | ResearchSourceClass::ForumThread
            | ResearchSourceClass::NewsArticle => Self::CommunityDiscussion,
            ResearchSourceClass::Unknown => Self::UnknownOrWeakContext,
        }
    }

    pub fn from_security_source_tier(sst: SecuritySourceTier) -> Self {
        match sst {
            SecuritySourceTier::PrimaryAdvisory | SecuritySourceTier::PackageRegistryAdvisory => {
                Self::AuthoritativeSecurityAdvisory
            }
            SecuritySourceTier::VendorAdvisory => Self::VendorSecurityGuidance,
            SecuritySourceTier::MaintainerDiscussion => Self::IssueOrIncidentDiscussion,
            SecuritySourceTier::ReleaseNotes => Self::ReleaseNoteOrChangelog,
            SecuritySourceTier::SecurityResearch => Self::IndependentCorroboration,
            SecuritySourceTier::NewsOrBlog | SecuritySourceTier::CommunityDiscussion => {
                Self::CommunityDiscussion
            }
            SecuritySourceTier::Unknown => Self::UnknownOrWeakContext,
        }
    }

    pub fn from_research_source_type(rst: ResearchSourceType) -> Self {
        match rst {
            ResearchSourceType::PrimarySources | ResearchSourceType::AcademicOrFormalSources => {
                Self::IndependentCorroboration
            }
            ResearchSourceType::OfficialDocs => Self::OfficialDocumentation,
            ResearchSourceType::Specifications => Self::InterfaceOrApiDefinition,
            ResearchSourceType::ReferenceImplementations => Self::PrimaryImplementation,
            ResearchSourceType::DesignDiscussions => Self::PullRequestOrDesignReview,
            ResearchSourceType::Benchmarks => Self::BenchmarkOrPerformanceEvidence,
            ResearchSourceType::SecurityConsiderations => Self::AuthoritativeSecurityAdvisory,
            ResearchSourceType::IssueThreads => Self::IssueOrIncidentDiscussion,
            ResearchSourceType::ReleaseNotes => Self::ReleaseNoteOrChangelog,
            ResearchSourceType::RecentNews | ResearchSourceType::CommunityDiscussion => {
                Self::CommunityDiscussion
            }
            ResearchSourceType::Counterpoints => Self::CounterpointOrConflictingEvidence,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::PrimaryImplementation => "Primary Implementation",
            Self::InterfaceOrApiDefinition => "Interface or API Definition",
            Self::UsageExample => "Usage Example",
            Self::TestOrBehavioralSpecification => "Test or Behavioral Specification",
            Self::ConfigurationOrFeatureGate => "Configuration or Feature Gate",
            Self::ManifestOrDependencyMetadata => "Manifest or Dependency Metadata",
            Self::OfficialDocumentation => "Official Documentation",
            Self::ArchitectureOrDesignDocument => "Architecture or Design Document",
            Self::ReleaseNoteOrChangelog => "Release Note or Changelog",
            Self::MigrationGuidance => "Migration Guidance",
            Self::BenchmarkOrPerformanceEvidence => "Benchmark or Performance Evidence",
            Self::IssueOrIncidentDiscussion => "Issue or Incident Discussion",
            Self::PullRequestOrDesignReview => "Pull Request or Design Review",
            Self::AuthoritativeSecurityAdvisory => "Authoritative Security Advisory",
            Self::VendorSecurityGuidance => "Vendor Security Guidance",
            Self::IndependentCorroboration => "Independent Corroboration",
            Self::CounterpointOrConflictingEvidence => "Counterpoint or Conflicting Evidence",
            Self::CommunityDiscussion => "Community Discussion",
            Self::UnknownOrWeakContext => "Unknown or Weak Context",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unknown_or_weak_context() {
        assert_eq!(EvidenceRole::default(), EvidenceRole::UnknownOrWeakContext);
    }

    #[test]
    fn label_returns_non_empty_for_all_variants() {
        let all = [
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
            EvidenceRole::ConfigurationOrFeatureGate,
            EvidenceRole::ManifestOrDependencyMetadata,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ArchitectureOrDesignDocument,
            EvidenceRole::ReleaseNoteOrChangelog,
            EvidenceRole::MigrationGuidance,
            EvidenceRole::BenchmarkOrPerformanceEvidence,
            EvidenceRole::IssueOrIncidentDiscussion,
            EvidenceRole::PullRequestOrDesignReview,
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
            EvidenceRole::IndependentCorroboration,
            EvidenceRole::CounterpointOrConflictingEvidence,
            EvidenceRole::CommunityDiscussion,
            EvidenceRole::UnknownOrWeakContext,
        ];
        for role in all {
            let label = role.label();
            assert!(!label.is_empty(), "label empty for {role:?}");
        }
    }

    #[test]
    fn from_source_kind_all_variants() {
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Unknown),
            EvidenceRole::UnknownOrWeakContext
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::OfficialDocs),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::PackageRegistry),
            EvidenceRole::ManifestOrDependencyMetadata
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::SourceRepository),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::RepositoryRoot),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::SourceDirectory),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::SourceFile),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Commit),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::IssueThread),
            EvidenceRole::IssueOrIncidentDiscussion
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::PullRequest),
            EvidenceRole::PullRequestOrDesignReview
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::ReleaseNotes),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Tag),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::SecurityAdvisory),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Reference),
            EvidenceRole::InterfaceOrApiDefinition
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::News),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Tutorial),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_source_kind(SourceKind::Forum),
            EvidenceRole::CommunityDiscussion
        );
    }

    #[test]
    fn from_source_role_all_variants() {
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Implementation),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Test),
            EvidenceRole::TestOrBehavioralSpecification
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Example),
            EvidenceRole::UsageExample
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Benchmark),
            EvidenceRole::BenchmarkOrPerformanceEvidence
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Configuration),
            EvidenceRole::ConfigurationOrFeatureGate
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Build),
            EvidenceRole::ConfigurationOrFeatureGate
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Documentation),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Readme),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Changelog),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Migration),
            EvidenceRole::MigrationGuidance
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Manifest),
            EvidenceRole::ManifestOrDependencyMetadata
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Lockfile),
            EvidenceRole::ManifestOrDependencyMetadata
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::SecurityPolicy),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Ci),
            EvidenceRole::ConfigurationOrFeatureGate
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Generated),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Vendor),
            EvidenceRole::UnknownOrWeakContext
        );
        assert_eq!(
            EvidenceRole::from_source_role(SourceRole::Unknown),
            EvidenceRole::UnknownOrWeakContext
        );
    }

    #[test]
    fn from_research_source_class_all_variants() {
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::OfficialDocs),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::ReferenceDocs),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::RepositorySource),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::MaintainerIssue),
            EvidenceRole::IssueOrIncidentDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::ReleaseNotes),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::Benchmark),
            EvidenceRole::BenchmarkOrPerformanceEvidence
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::Paper),
            EvidenceRole::IndependentCorroboration
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::StandardSpec),
            EvidenceRole::InterfaceOrApiDefinition
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::SecurityAdvisory),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::VendorBlog),
            EvidenceRole::VendorSecurityGuidance
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::EngineeringBlog),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::ForumThread),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::NewsArticle),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_class(ResearchSourceClass::Unknown),
            EvidenceRole::UnknownOrWeakContext
        );
    }

    #[test]
    fn from_security_source_tier_all_variants() {
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::PrimaryAdvisory),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::VendorAdvisory),
            EvidenceRole::VendorSecurityGuidance
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::PackageRegistryAdvisory),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::MaintainerDiscussion),
            EvidenceRole::IssueOrIncidentDiscussion
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::ReleaseNotes),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::SecurityResearch),
            EvidenceRole::IndependentCorroboration
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::NewsOrBlog),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::CommunityDiscussion),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_security_source_tier(SecuritySourceTier::Unknown),
            EvidenceRole::UnknownOrWeakContext
        );
    }

    #[test]
    fn from_research_source_type_all_variants() {
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::PrimarySources),
            EvidenceRole::IndependentCorroboration
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::OfficialDocs),
            EvidenceRole::OfficialDocumentation
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::Specifications),
            EvidenceRole::InterfaceOrApiDefinition
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::ReferenceImplementations),
            EvidenceRole::PrimaryImplementation
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::DesignDiscussions),
            EvidenceRole::PullRequestOrDesignReview
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::Benchmarks),
            EvidenceRole::BenchmarkOrPerformanceEvidence
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::SecurityConsiderations),
            EvidenceRole::AuthoritativeSecurityAdvisory
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::IssueThreads),
            EvidenceRole::IssueOrIncidentDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::ReleaseNotes),
            EvidenceRole::ReleaseNoteOrChangelog
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::AcademicOrFormalSources),
            EvidenceRole::IndependentCorroboration
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::RecentNews),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::CommunityDiscussion),
            EvidenceRole::CommunityDiscussion
        );
        assert_eq!(
            EvidenceRole::from_research_source_type(ResearchSourceType::Counterpoints),
            EvidenceRole::CounterpointOrConflictingEvidence
        );
    }

    #[test]
    fn serde_roundtrip() {
        let roles = [
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
            EvidenceRole::ConfigurationOrFeatureGate,
            EvidenceRole::ManifestOrDependencyMetadata,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ArchitectureOrDesignDocument,
            EvidenceRole::ReleaseNoteOrChangelog,
            EvidenceRole::MigrationGuidance,
            EvidenceRole::BenchmarkOrPerformanceEvidence,
            EvidenceRole::IssueOrIncidentDiscussion,
            EvidenceRole::PullRequestOrDesignReview,
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
            EvidenceRole::IndependentCorroboration,
            EvidenceRole::CounterpointOrConflictingEvidence,
            EvidenceRole::CommunityDiscussion,
            EvidenceRole::UnknownOrWeakContext,
        ];
        for role in roles {
            let json = serde_json::to_string(&role).unwrap();
            let parsed: EvidenceRole = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, role, "roundtrip failed for {role:?}");
        }
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_string(&EvidenceRole::PrimaryImplementation).unwrap();
        assert_eq!(json, "\"primary_implementation\"");
        let json = serde_json::to_string(&EvidenceRole::AuthoritativeSecurityAdvisory).unwrap();
        assert_eq!(json, "\"authoritative_security_advisory\"");
        let json = serde_json::to_string(&EvidenceRole::CounterpointOrConflictingEvidence).unwrap();
        assert_eq!(json, "\"counterpoint_or_conflicting_evidence\"");
    }

    #[test]
    fn serde_deserializes_snake_case() {
        let role: EvidenceRole = serde_json::from_str("\"usage_example\"").unwrap();
        assert_eq!(role, EvidenceRole::UsageExample);
        let role: EvidenceRole = serde_json::from_str("\"unknown_or_weak_context\"").unwrap();
        assert_eq!(role, EvidenceRole::UnknownOrWeakContext);
    }

    #[test]
    fn hash_consistent_with_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a = EvidenceRole::PrimaryImplementation;
        let b = EvidenceRole::PrimaryImplementation;
        assert_eq!(a, b);

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        a.hash(&mut h1);
        b.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn labels_are_unique() {
        let roles = [
            EvidenceRole::PrimaryImplementation,
            EvidenceRole::InterfaceOrApiDefinition,
            EvidenceRole::UsageExample,
            EvidenceRole::TestOrBehavioralSpecification,
            EvidenceRole::ConfigurationOrFeatureGate,
            EvidenceRole::ManifestOrDependencyMetadata,
            EvidenceRole::OfficialDocumentation,
            EvidenceRole::ArchitectureOrDesignDocument,
            EvidenceRole::ReleaseNoteOrChangelog,
            EvidenceRole::MigrationGuidance,
            EvidenceRole::BenchmarkOrPerformanceEvidence,
            EvidenceRole::IssueOrIncidentDiscussion,
            EvidenceRole::PullRequestOrDesignReview,
            EvidenceRole::AuthoritativeSecurityAdvisory,
            EvidenceRole::VendorSecurityGuidance,
            EvidenceRole::IndependentCorroboration,
            EvidenceRole::CounterpointOrConflictingEvidence,
            EvidenceRole::CommunityDiscussion,
            EvidenceRole::UnknownOrWeakContext,
        ];
        let labels: Vec<&str> = roles.iter().map(|r| r.label()).collect();
        let unique: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(labels.len(), unique.len(), "duplicate labels found");
    }
}
