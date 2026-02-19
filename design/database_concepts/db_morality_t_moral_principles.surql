USE NS project_universe;
USE DB game;

DEFINE TABLE moral_principles SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD principle_name    ON TABLE moral_principles TYPE string;
DEFINE FIELD description       ON TABLE moral_principles TYPE string;
DEFINE FIELD ethical_system    ON TABLE moral_principles TYPE string;

DEFINE INDEX unique_principle_name ON TABLE moral_principles COLUMNS principle_name UNIQUE;

-- Universal Ethics Principles
CREATE moral_principles:golden_rule SET
  principle_name = "The Golden Rule",
  description = "Treat others as you would want to be treated.",
  ethical_system = "Universal Ethics";

CREATE moral_principles:universalizability SET
  principle_name = "Universalizability",
  description = "Act only according to maxims that could be universal laws.",
  ethical_system = "Universal Ethics";

CREATE moral_principles:human_dignity SET
  principle_name = "Human Dignity",
  description = "Every human being has inherent worth and deserves respect.",
  ethical_system = "Universal Ethics";

-- Consequentialist Principles
CREATE moral_principles:utilitarianism SET
  principle_name = "Utilitarianism",
  description = "Actions are right if they promote the greatest happiness for the greatest number.",
  ethical_system = "Consequentialism";

CREATE moral_principles:cost_benefit SET
  principle_name = "Cost-Benefit Analysis",
  description = "Evaluate actions based on their overall benefits and harms.",
  ethical_system = "Consequentialism";

CREATE moral_principles:preference_utilitarianism SET
  principle_name = "Preference Utilitarianism",
  description = "Actions should maximize the satisfaction of preferences.",
  ethical_system = "Consequentialism";

-- Deontological Principles
CREATE moral_principles:kantianism SET
  principle_name = "Kantianism",
  description = "Act only according to that maxim by which you can at the same time will that it should become a universal law.",
  ethical_system = "Deontology";

CREATE moral_principles:duty_based SET
  principle_name = "Duty-Based Ethics",
  description = "Actions are morally right if they follow moral rules or duties.",
  ethical_system = "Deontology";

CREATE moral_principles:rights_based SET
  principle_name = "Rights-Based Ethics",
  description = "Actions are right if they respect and protect moral rights.",
  ethical_system = "Deontology";

-- Virtue Ethics Principles
CREATE moral_principles:virtue_ethics SET
  principle_name = "Virtue Ethics",
  description = "Focus on character and virtues rather than rules or consequences.",
  ethical_system = "Virtue Ethics";

CREATE moral_principles:character_development SET
  principle_name = "Character Development",
  description = "Cultivate virtuous character traits for moral excellence.",
  ethical_system = "Virtue Ethics";

CREATE moral_principles:phronesis SET
  principle_name = "Phronesis",
  description = "Practical wisdom in applying virtues to specific situations.",
  ethical_system = "Virtue Ethics";

-- Rights-Based Ethics Principles
CREATE moral_principles:human_rights SET
  principle_name = "Human Rights",
  description = "Respect for fundamental human rights and freedoms.",
  ethical_system = "Rights-Based Ethics";

CREATE moral_principles:autonomy SET
  principle_name = "Autonomy",
  description = "Respect for individual self-determination and choice.",
  ethical_system = "Rights-Based Ethics";

CREATE moral_principles:justice SET
  principle_name = "Justice",
  description = "Fair and equitable treatment of all individuals.",
  ethical_system = "Rights-Based Ethics";

-- Care Ethics Principles
CREATE moral_principles:care_ethics SET
  principle_name = "Care Ethics",
  description = "Focus on relationships and caring for others.",
  ethical_system = "Care Ethics";

CREATE moral_principles:relational_responsibility SET
  principle_name = "Relational Responsibility",
  description = "Moral obligations arise from relationships and connections.",
  ethical_system = "Care Ethics";

CREATE moral_principles:empathy SET
  principle_name = "Empathy",
  description = "Understanding and sharing the feelings of others.",
  ethical_system = "Care Ethics";

-- Environmental Ethics Principles
CREATE moral_principles:environmental_stewardship SET
  principle_name = "Environmental Stewardship",
  description = "Responsibility to protect and preserve the environment.",
  ethical_system = "Environmental Ethics";

CREATE moral_principles:sustainability SET
  principle_name = "Sustainability",
  description = "Meeting present needs without compromising future generations.",
  ethical_system = "Environmental Ethics";

CREATE moral_principles:biocentric_equality SET
  principle_name = "Biocentric Equality",
  description = "All living things have equal moral standing.",
  ethical_system = "Environmental Ethics";

-- Professional Ethics Principles
CREATE moral_principles:professional_responsibility SET
  principle_name = "Professional Responsibility",
  description = "Obligations specific to professional roles and standards.",
  ethical_system = "Professional Ethics";

CREATE moral_principles:confidentiality SET
  principle_name = "Confidentiality",
  description = "Protection of private information and trust.",
  ethical_system = "Professional Ethics";

CREATE moral_principles:competence SET
  principle_name = "Competence",
  description = "Maintaining professional skills and knowledge.",
  ethical_system = "Professional Ethics"; 