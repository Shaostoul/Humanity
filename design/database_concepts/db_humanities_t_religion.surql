USE NS project_universe;
USE DB humanities;
  
DEFINE TABLE religion SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE religion TYPE string;
DEFINE FIELD description     ON TABLE religion TYPE string;
DEFINE FIELD type           ON TABLE religion TYPE string;
DEFINE FIELD principle      ON TABLE religion TYPE string;
DEFINE FIELD components     ON TABLE religion TYPE array;
DEFINE FIELD requirements   ON TABLE religion TYPE array;
DEFINE FIELD characteristics ON TABLE religion TYPE object;
DEFINE FIELD variations     ON TABLE religion TYPE array;
DEFINE FIELD applications   ON TABLE religion TYPE array;
DEFINE FIELD related_systems ON TABLE religion TYPE array;
  
DEFINE INDEX unique_religious_system ON TABLE religion COLUMNS name UNIQUE;
DEFINE INDEX religious_type ON TABLE religion COLUMNS type;
  
-- Traditional Religion
CREATE religion:traditional_religion SET
  name = "Traditional Religious System",
  description = "Established religious beliefs and practices",
  type = "Traditional",
  principle = "Faith-based belief system with established traditions",
  components = ["Beliefs", "Rituals", "Sacred texts", "Religious authorities"],
  requirements = ["Faith", "Community participation", "Religious education", "Cultural context"],
  characteristics = {
    stability: "High",
    structure: "Strong",
    tradition: "Deep",
    community: "Strong"
  },
  variations = ["Denominations", "Sects", "Cultural adaptations", "Historical developments"],
  applications = ["Spiritual guidance", "Community building", "Cultural preservation"],
  related_systems = ["culture:traditional_culture", "philosophy:ethics"];
  
-- Modern Religion
CREATE religion:modern_religion SET
  name = "Modern Religious System",
  description = "Contemporary religious expressions and practices",
  type = "Modern",
  principle = "Adaptation of religious beliefs to modern context",
  components = ["Contemporary beliefs", "Modern practices", "Digital presence", "Social engagement"],
  requirements = ["Modern context", "Digital literacy", "Social awareness", "Adaptability"],
  characteristics = {
    adaptability: "High",
    innovation: "Present",
    inclusivity: "High",
    engagement: "Active"
  },
  variations = ["Progressive movements", "Digital communities", "Interfaith initiatives", "Social activism"],
  applications = ["Modern spiritual guidance", "Digital community building", "Social engagement"],
  related_systems = ["culture:modern_culture", "language:digital_communication"];
  
-- Interfaith System
CREATE religion:interfaith_system SET
  name = "Interfaith Religious System",
  description = "Integration of multiple religious traditions",
  type = "Interfaith",
  principle = "Harmonious coexistence of religious beliefs",
  components = ["Multiple traditions", "Shared values", "Dialogue platforms", "Common practices"],
  requirements = ["Religious literacy", "Tolerance", "Open-mindedness", "Communication skills"],
  characteristics = {
    inclusivity: "Very high",
    complexity: "High",
    harmony: "Essential",
    growth: "Continuous"
  },
  variations = ["Interfaith initiatives", "Shared ceremonies", "Combined practices", "Cultural fusion"],
  applications = ["Religious harmony", "Cultural understanding", "Global peace"],
  related_systems = ["culture:hybrid_culture", "society:social_structure"];
  
-- Space Religion
CREATE religion:space_religion SET
  name = "Space-Adapted Religious System",
  description = "Religious practices adapted for space environments",
  type = "Space",
  principle = "Adaptation of religious practices to space conditions",
  components = ["Space-adapted rituals", "Microgravity ceremonies", "Digital worship", "Universal themes"],
  requirements = ["Space adaptation", "Technological integration", "Cultural sensitivity", "Psychological support"],
  characteristics = {
    adaptability: "Very high",
    innovation: "High",
    universality: "High",
    resilience: "Strong"
  },
  variations = ["Space-specific rituals", "Digital worship", "Interplanetary faith", "Universal spirituality"],
  applications = ["Space mission support", "Psychological well-being", "Community building"],
  related_systems = ["culture:space_culture", "society:space_community"]; 