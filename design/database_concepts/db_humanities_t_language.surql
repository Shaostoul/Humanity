USE NS project_universe;
USE DB humanities;
  
DEFINE TABLE language SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE language TYPE string;
DEFINE FIELD description     ON TABLE language TYPE string;
DEFINE FIELD type           ON TABLE language TYPE string;
DEFINE FIELD principle      ON TABLE language TYPE string;
DEFINE FIELD components     ON TABLE language TYPE array;
DEFINE FIELD requirements   ON TABLE language TYPE array;
DEFINE FIELD characteristics ON TABLE language TYPE object;
DEFINE FIELD variations     ON TABLE language TYPE array;
DEFINE FIELD applications   ON TABLE language TYPE array;
DEFINE FIELD related_systems ON TABLE language TYPE array;
  
DEFINE INDEX unique_language_system ON TABLE language COLUMNS name UNIQUE;
DEFINE INDEX language_type ON TABLE language COLUMNS type;
  
-- Spoken Language
CREATE language:spoken_language SET
  name = "Spoken Language System",
  description = "Traditional vocal communication system",
  type = "Vocal",
  principle = "Sound-based communication through speech",
  components = ["Phonetics", "Grammar", "Vocabulary", "Pragmatics"],
  requirements = ["Vocal apparatus", "Auditory perception", "Language acquisition", "Social interaction"],
  characteristics = {
    complexity: "High",
    expressiveness: "High",
    learning_curve: "Steep",
    universality: "High"
  },
  variations = ["Dialects", "Accents", "Registers", "Idiolects"],
  applications = ["Face-to-face communication", "Oral storytelling", "Social interaction"],
  related_systems = ["culture:traditional_culture", "society:social_structure"];
  
-- Digital Communication
CREATE language:digital_communication SET
  name = "Digital Communication System",
  description = "Modern electronic communication system",
  type = "Digital",
  principle = "Electronic transmission of information",
  components = ["Text messaging", "Social media", "Email", "Digital platforms"],
  requirements = ["Digital devices", "Network access", "Literacy", "Technical skills"],
  characteristics = {
    speed: "Very high",
    reach: "Global",
    permanence: "Variable",
    formality: "Variable"
  },
  variations = ["Platform-specific conventions", "Generational differences", "Cultural adaptations"],
  applications = ["Remote communication", "Social networking", "Information sharing"],
  related_systems = ["culture:modern_culture", "society:social_network"];
  
-- Multilingual System
CREATE language:multilingual_system SET
  name = "Multilingual Communication System",
  description = "Integration of multiple languages and communication methods",
  type = "Hybrid",
  principle = "Combined use of multiple communication systems",
  components = ["Multiple languages", "Translation systems", "Cultural adaptation", "Code-switching"],
  requirements = ["Language proficiency", "Cultural awareness", "Translation tools", "Adaptability"],
  characteristics = {
    flexibility: "High",
    complexity: "Very high",
    inclusivity: "High",
    learning_curve: "Steep"
  },
  variations = ["Language combinations", "Proficiency levels", "Cultural contexts"],
  applications = ["International communication", "Cultural exchange", "Global collaboration"],
  related_systems = ["culture:hybrid_culture", "education:language_training"];
  
-- Space Communication
CREATE language:space_communication SET
  name = "Space Communication System",
  description = "Specialized communication system for space environments",
  type = "Space",
  principle = "Adapted communication for space conditions",
  components = ["Technical protocols", "Emergency procedures", "Multilingual support", "Non-verbal cues"],
  requirements = ["Technical training", "Protocol adherence", "Stress management", "Team coordination"],
  characteristics = {
    clarity: "Very high",
    efficiency: "High",
    reliability: "Critical",
    adaptability: "High"
  },
  variations = ["Mission-specific protocols", "Crew dynamics", "Emergency procedures"],
  applications = ["Space mission communication", "Emergency response", "Team coordination"],
  related_systems = ["culture:space_culture", "society:space_community"]; 