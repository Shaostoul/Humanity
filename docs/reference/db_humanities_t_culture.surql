USE NS project_universe;
USE DB humanities;
  
DEFINE TABLE culture SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE culture TYPE string;
DEFINE FIELD description     ON TABLE culture TYPE string;
DEFINE FIELD type           ON TABLE culture TYPE string;
DEFINE FIELD principle      ON TABLE culture TYPE string;
DEFINE FIELD components     ON TABLE culture TYPE array;
DEFINE FIELD requirements   ON TABLE culture TYPE array;
DEFINE FIELD characteristics ON TABLE culture TYPE object;
DEFINE FIELD variations     ON TABLE culture TYPE array;
DEFINE FIELD applications   ON TABLE culture TYPE array;
DEFINE FIELD related_systems ON TABLE culture TYPE array;
  
DEFINE INDEX unique_cultural_system ON TABLE culture COLUMNS name UNIQUE;
DEFINE INDEX cultural_type ON TABLE culture COLUMNS type;
  
-- Traditional Culture
CREATE culture:traditional_culture SET
  name = "Traditional Cultural System",
  description = "Established cultural practices and traditions",
  type = "Traditional",
  principle = "Preservation and transmission of cultural heritage",
  components = ["Customs", "Traditions", "Rituals", "Social norms"],
  requirements = ["Community participation", "Cultural knowledge", "Social structure", "Transmission methods"],
  characteristics = {
    stability: "High",
    adaptability: "Low to moderate",
    complexity: "High",
    integration: "Strong"
  },
  variations = ["Regional differences", "Social class variations", "Generational changes"],
  applications = ["Social cohesion", "Identity formation", "Cultural preservation"],
  related_systems = ["language:spoken_language", "religion:traditional_religion"];
  
-- Modern Culture
CREATE culture:modern_culture SET
  name = "Modern Cultural System",
  description = "Contemporary cultural practices and expressions",
  type = "Modern",
  principle = "Adaptation and innovation in cultural expression",
  components = ["Technology", "Media", "Social networks", "Global influences"],
  requirements = ["Access to technology", "Social connectivity", "Cultural exchange", "Innovation"],
  characteristics = {
    stability: "Moderate",
    adaptability: "High",
    complexity: "Very high",
    integration: "Variable"
  },
  variations = ["Digital divide", "Generational gaps", "Global-local tensions"],
  applications = ["Social networking", "Cultural innovation", "Global communication"],
  related_systems = ["language:digital_communication", "religion:modern_religion"];
  
-- Hybrid Culture
CREATE culture:hybrid_culture SET
  name = "Hybrid Cultural System",
  description = "Integration of traditional and modern cultural elements",
  type = "Hybrid",
  principle = "Synthesis of cultural traditions and innovations",
  components = ["Traditional elements", "Modern adaptations", "Cultural fusion", "Identity markers"],
  requirements = ["Cultural awareness", "Adaptability", "Social acceptance", "Integration methods"],
  characteristics = {
    stability: "Moderate to high",
    adaptability: "High",
    complexity: "High",
    integration: "Strong"
  },
  variations = ["Cultural balance", "Integration levels", "Identity expression"],
  applications = ["Cultural evolution", "Social adaptation", "Identity formation"],
  related_systems = ["language:multilingual_system", "religion:interfaith_system"];
  
-- Space Culture
CREATE culture:space_culture SET
  name = "Space-Adapted Cultural System",
  description = "Cultural practices adapted for space environments",
  type = "Space",
  principle = "Adaptation of cultural practices to space conditions",
  components = ["Space-adapted rituals", "Microgravity customs", "Isolation coping", "Community bonding"],
  requirements = ["Environmental adaptation", "Psychological support", "Social cohesion", "Cultural continuity"],
  characteristics = {
    stability: "High",
    adaptability: "Very high",
    complexity: "High",
    integration: "Strong"
  },
  variations = ["Mission-specific adaptations", "Crew dynamics", "Cultural backgrounds"],
  applications = ["Space mission support", "Psychological well-being", "Community building"],
  related_systems = ["language:space_communication", "religion:space_religion"];
