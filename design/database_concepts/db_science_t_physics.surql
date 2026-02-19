USE NS project_universe;
USE DB science;

DEFINE TABLE physics SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE physics TYPE string;
DEFINE FIELD description     ON TABLE physics TYPE string;
DEFINE FIELD law_formula     ON TABLE physics TYPE string;
DEFINE FIELD variables       ON TABLE physics TYPE array;
DEFINE FIELD units           ON TABLE physics TYPE array;
DEFINE FIELD applications    ON TABLE physics TYPE array;
DEFINE FIELD limitations     ON TABLE physics TYPE array;
DEFINE FIELD related_laws    ON TABLE physics TYPE array;

DEFINE INDEX unique_physics_law ON TABLE physics COLUMNS name UNIQUE;

-- Newton's Laws of Motion
CREATE physics:newtons_first SET
  name = "Newton's First Law",
  description = "An object at rest stays at rest, and an object in motion stays in motion unless acted upon by an external force",
  law_formula = "F = 0 when v is constant",
  variables = ["F", "v"],
  units = ["N", "m/s"],
  applications = ["Spacecraft Inertial Motion", "Orbital Mechanics"],
  limitations = ["Does not apply in non-inertial reference frames"],
  related_laws = ["physics:newtons_second", "physics:newtons_third"];

CREATE physics:newtons_second SET
  name = "Newton's Second Law",
  description = "The acceleration of an object is directly proportional to the net force acting on it and inversely proportional to its mass",
  law_formula = "F = ma",
  variables = ["F", "m", "a"],
  units = ["N", "kg", "m/s²"],
  applications = ["Rocket Propulsion", "Spacecraft Maneuvering"],
  limitations = ["Assumes constant mass"],
  related_laws = ["physics:newtons_first", "physics:newtons_third"];

CREATE physics:newtons_third SET
  name = "Newton's Third Law",
  description = "For every action, there is an equal and opposite reaction",
  law_formula = "F₁₂ = -F₂₁",
  variables = ["F₁₂", "F₂₁"],
  units = ["N"],
  applications = ["Rocket Propulsion", "Spacecraft Docking"],
  limitations = ["Forces must be of the same type"],
  related_laws = ["physics:newtons_first", "physics:newtons_second"];

-- Conservation Laws
CREATE physics:conservation_energy SET
  name = "Conservation of Energy",
  description = "Energy cannot be created or destroyed, only transformed from one form to another",
  law_formula = "E₁ = E₂",
  variables = ["E₁", "E₂"],
  units = ["J"],
  applications = ["Power Systems", "Propulsion Efficiency"],
  limitations = ["Assumes isolated system"],
  related_laws = ["physics:conservation_momentum", "physics:conservation_mass"];

CREATE physics:conservation_momentum SET
  name = "Conservation of Momentum",
  description = "The total momentum of an isolated system remains constant",
  law_formula = "p₁ = p₂",
  variables = ["p₁", "p₂"],
  units = ["kg⋅m/s"],
  applications = ["Collision Analysis", "Spacecraft Rendezvous"],
  limitations = ["Assumes isolated system"],
  related_laws = ["physics:conservation_energy", "physics:conservation_mass"];

-- Thermodynamics
CREATE physics:first_law_thermodynamics SET
  name = "First Law of Thermodynamics",
  description = "The change in internal energy of a system equals the heat added minus the work done",
  law_formula = "ΔU = Q - W",
  variables = ["ΔU", "Q", "W"],
  units = ["J"],
  applications = ["Life Support Systems", "Heat Management"],
  limitations = ["Assumes closed system"],
  related_laws = ["physics:second_law_thermodynamics"];

CREATE physics:second_law_thermodynamics SET
  name = "Second Law of Thermodynamics",
  description = "The total entropy of an isolated system always increases over time",
  law_formula = "ΔS ≥ 0",
  variables = ["ΔS"],
  units = ["J/K"],
  applications = ["Heat Engine Design", "Thermal Management"],
  limitations = ["Applies to macroscopic systems"],
  related_laws = ["physics:first_law_thermodynamics"];

-- Electromagnetism
CREATE physics:maxwell_equations SET
  name = "Maxwell's Equations",
  description = "Set of four equations describing the behavior of electric and magnetic fields",
  law_formula = "∇⋅E = ρ/ε₀, ∇⋅B = 0, ∇×E = -∂B/∂t, ∇×B = μ₀J + μ₀ε₀∂E/∂t",
  variables = ["E", "B", "ρ", "J", "ε₀", "μ₀"],
  units = ["N/C", "T", "C/m³", "A/m²", "F/m", "H/m"],
  applications = ["Communication Systems", "Power Generation"],
  limitations = ["Classical approximation"],
  related_laws = ["physics:ohms_law"];

CREATE physics:ohms_law SET
  name = "Ohm's Law",
  description = "The current through a conductor is directly proportional to the voltage across it",
  law_formula = "V = IR",
  variables = ["V", "I", "R"],
  units = ["V", "A", "Ω"],
  applications = ["Circuit Design", "Power Systems"],
  limitations = ["Applies to ohmic materials"],
  related_laws = ["physics:maxwell_equations"];
