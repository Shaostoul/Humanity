USE NS project_universe;
USE DB lifeforms;

DEFINE TABLE actions SCHEMAFULL
  PERMISSIONS
    FOR create WHERE $auth.role == "admin"
    FOR select WHERE true
    FOR update WHERE $auth.role == "admin"
    FOR delete WHERE $auth.role == "admin";

DEFINE FIELD name           ON TABLE actions TYPE string ASSERT $value != NONE;
DEFINE FIELD description    ON TABLE actions TYPE string ASSERT $value != NONE;

DEFINE INDEX idx_actions_name ON TABLE actions COLUMNS name UNIQUE;

-- Fundamental mechanical actions
CREATE actions:cutting SET
  name = "Cutting",
  description = "Dividing materials into smaller pieces using mechanical or manual force.";

CREATE actions:shredding SET
  name = "Shredding",
  description = "Reducing materials into flakes or small pieces using mechanical blades or manual tools.";

CREATE actions:stripping SET
  name = "Stripping",
  description = "Removing layers or strips from materials, such as stripping wires or plastic bottles.";

CREATE actions:grinding SET
  name = "Grinding",
  description = "Reducing materials to powder or small granules through abrasive action.";

CREATE actions:crushing SET
  name = "Crushing",
  description = "Applying force to compress materials into smaller fragments or particles.";

CREATE actions:drilling SET
  name = "Drilling",
  description = "Creating holes or channels in materials.";

CREATE actions:assembling SET
  name = "Assembling",
  description = "Combining components into a finished product or structure.";

CREATE actions:extruding SET
  name = "Extruding",
  description = "Forcing materials through a shaped opening to produce a continuous form.";

-- Thermal and heat-related actions
CREATE actions:heating SET
  name = "Heating",
  description = "Applying heat to increase temperature of materials.";

CREATE actions:melting SET
  name = "Melting",
  description = "Heating materials until they transition from solid to liquid.";

CREATE actions:drying SET
  name = "Drying",
  description = "Removing moisture or liquids through evaporation.";

CREATE actions:cooling SET
  name = "Cooling",
  description = "Reducing material temperature, typically solidifying liquids or preserving materials.";

CREATE actions:sintering SET
  name = "Sintering",
  description = "Fusing particles of material together by heating without fully melting.";

-- Chemical and fluid-based actions
CREATE actions:mixing SET
  name = "Mixing",
  description = "Combining multiple materials evenly.";

CREATE actions:dissolving SET
  name = "Dissolving",
  description = "Mixing a substance into a solvent to form a homogeneous solution.";

CREATE actions:fermenting SET
  name = "Fermenting",
  description = "Biological breakdown of substances using microorganisms.";

CREATE actions:distilling SET
  name = "Distilling",
  description = "Separating substances by evaporating and condensing based on boiling points.";

CREATE actions:filtering SET
  name = "Filtering",
  description = "Separating solids from liquids or gases by passing them through a porous barrier.";

CREATE actions:purifying SET
  name = "Purifying",
  description = "Removing impurities or contaminants from materials.";

-- Electrical and electrochemical actions
CREATE actions:electrolysis SET
  name = "Electrolysis",
  description = "Using electricity to drive a chemical reaction, typically for separating elements or compounds.";

CREATE actions:charging SET
  name = "Charging",
  description = "Storing electrical energy in batteries or capacitors.";

CREATE actions:discharging SET
  name = "Discharging",
  description = "Releasing stored electrical energy from batteries or capacitors.";

CREATE actions:welding SET
  name = "Welding",
  description = "Joining metals by melting them together through electrical or thermal processes.";

-- Biological and ecological actions
CREATE actions:composting SET
  name = "Composting",
  description = "Biological decomposition of organic matter into nutrient-rich compost.";

CREATE actions:growing SET
  name = "Growing",
  description = "Cultivating plants, fungi, or other biological organisms.";

CREATE actions:harvesting SET
  name = "Harvesting",
  description = "Collecting mature biological products from plants or animals.";

CREATE actions:breeding SET
  name = "Breeding",
  description = "Controlled reproduction of animals or plants for desired traits.";

-- Inspection and analysis actions
CREATE actions:measuring SET
  name = "Measuring",
  description = "Quantitatively determining properties or dimensions of materials.";

CREATE actions:analyzing SET
  name = "Analyzing",
  description = "Examining materials or data to identify properties, composition, or quality.";

CREATE actions:testing SET
  name = "Testing",
  description = "Performing evaluations or experiments to verify functionality or quality of materials or equipment.";
