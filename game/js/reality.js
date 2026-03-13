  // ══════════════════════════════════════
  // REALITY TAB — Todo, Notes, Garden, Catalogs
  // ══════════════════════════════════════

  // ── Card collapse ──
  const REALITY_CARDS = ['todo', 'notes', 'garden', 'references', 'elements', 'materials', 'inventory', 'assets'];
  function toggleRealityCard(id) {
   const card = document.getElementById('reality-' + id);
   if (card) card.classList.toggle('collapsed');
   localStorage.setItem('reality_collapsed_' + id, card.classList.contains('collapsed'));
  }
  // Restore collapse state
  REALITY_CARDS.forEach(id => {
   if (localStorage.getItem('reality_collapsed_' + id) === 'true') {
    const card = document.getElementById('reality-' + id);
    if (card) card.classList.add('collapsed');
   }
  });

  // ══════════════════════════════════════
  // REFERENCE SOURCES
  // ══════════════════════════════════════
  const REF_SOURCES = [
   { icon: '⚛️', name: 'NIST', url: 'https://nist.gov', desc: 'Physical constants, element data, standards' },
   { icon: '🧪', name: 'PubChem', url: 'https://pubchem.ncbi.nlm.nih.gov', desc: 'Chemical compounds database' },
   { icon: '🔬', name: 'PubMed', url: 'https://pubmed.ncbi.nlm.nih.gov', desc: 'Biomedical research papers' },
   { icon: '🌌', name: 'NASA ADS', url: 'https://ui.adsabs.harvard.edu', desc: 'Astrophysics data system' },
   { icon: '📖', name: 'Wikipedia', url: 'https://wikipedia.org', desc: 'General reference encyclopedia' },
   { icon: '🪨', name: 'USGS', url: 'https://usgs.gov', desc: 'Geological surveys, minerals, mines' },
   { icon: '🏗️', name: 'MatWeb', url: 'https://matweb.com', desc: 'Material property data' },
   { icon: 'Quality', name: 'Wolfram Alpha', url: 'https://wolframalpha.com', desc: 'Computational knowledge engine' },
  ];
  (function renderRefSources() {
   document.getElementById('ref-sources-grid').innerHTML = REF_SOURCES.map(s =>
    `<a href="${s.url}" target="_blank" rel="noopener" class="ref-card">
     <span class="ref-icon">${s.icon}</span>
     <div><div class="ref-name">${s.name}</div><div class="ref-desc">${s.desc}</div></div>
    </a>`
   ).join('');
  })();

  // ══════════════════════════════════════
  // ELEMENT CATALOG
  // ══════════════════════════════════════
  const ELEMENTS = [
   { symbol:'H', name:'Hydrogen', number:1, category:'nonmetal', mass:1.008, phase:'Gas',
    density:'0.00008988 g/cm³', meltingPoint:'-259.16 °C', boilingPoint:'-252.87 °C',
    discovered:'1766', discoverer:'Henry Cavendish',
    uses:['Fuel cells','Ammonia production','Rocket fuel','Hydrogenation'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C1333740'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Hydrogen'}],
    description:'Lightest element. Most abundant chemical substance in the universe.' },
   { symbol:'C', name:'Carbon', number:6, category:'nonmetal', mass:12.011, phase:'Solid',
    density:'2.267 g/cm³', meltingPoint:'3550 °C', boilingPoint:'4027 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Steel production','Fuels','Plastics','Diamond/graphite'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440440'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Carbon'}],
    description:'Basis of all known life. Forms more compounds than any other element.' },
   { symbol:'N', name:'Nitrogen', number:7, category:'nonmetal', mass:14.007, phase:'Gas',
    density:'0.0012506 g/cm³', meltingPoint:'-210.00 °C', boilingPoint:'-195.79 °C',
    discovered:'1772', discoverer:'Daniel Rutherford',
    uses:['Fertilizers','Explosives','Cryogenics','Food preservation'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7727379'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Nitrogen'}],
    description:'Makes up 78% of Earth\'s atmosphere. Essential for amino acids and DNA.' },
   { symbol:'O', name:'Oxygen', number:8, category:'nonmetal', mass:15.999, phase:'Gas',
    density:'0.001429 g/cm³', meltingPoint:'-218.79 °C', boilingPoint:'-182.96 °C',
    discovered:'1774', discoverer:'Joseph Priestley',
    uses:['Respiration','Steel manufacturing','Medical','Welding'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7782447'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Oxygen'}],
    description:'Essential for aerobic life. Third most abundant element in the universe.' },
   { symbol:'Si', name:'Silicon', number:14, category:'metalloid', mass:28.085, phase:'Solid',
    density:'2.3296 g/cm³', meltingPoint:'1414 °C', boilingPoint:'3265 °C',
    discovered:'1824', discoverer:'Jöns Jacob Berzelius',
    uses:['Semiconductors','Glass','Concrete','Solar cells'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440213'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Silicon'}],
    description:'Second most abundant element in Earth\'s crust. Foundation of modern electronics.' },
   { symbol:'Fe', name:'Iron', number:26, category:'transition-metal', mass:55.845, phase:'Solid',
    density:'7.874 g/cm³', meltingPoint:'1538 °C', boilingPoint:'2862 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Steel','Construction','Machinery','Magnets'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439896'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Iron'}],
    description:'Most common element on Earth by mass. Core component of steel.' },
   { symbol:'Cu', name:'Copper', number:29, category:'transition-metal', mass:63.546, phase:'Solid',
    density:'8.96 g/cm³', meltingPoint:'1084.62 °C', boilingPoint:'2562 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Electrical wiring','Plumbing','Electronics','Alloys'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440508'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Copper'}],
    description:'Excellent conductor of electricity and heat. One of few metals with natural color.' },
   { symbol:'Ag', name:'Silver', number:47, category:'transition-metal', mass:107.868, phase:'Solid',
    density:'10.49 g/cm³', meltingPoint:'961.78 °C', boilingPoint:'2162 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Jewelry','Electronics','Photography','Antibacterial'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440224'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Silver'}],
    description:'Highest electrical and thermal conductivity of any element. Precious metal.' },
   { symbol:'Au', name:'Gold', number:79, category:'transition-metal', mass:196.967, phase:'Solid',
    density:'19.3 g/cm³', meltingPoint:'1064.18 °C', boilingPoint:'2856 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Currency/Investment','Jewelry','Electronics','Dentistry'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440575'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Gold'}],
    description:'Highly valued precious metal. Extremely malleable and resistant to corrosion.' },
   { symbol:'U', name:'Uranium', number:92, category:'actinide', mass:238.029, phase:'Solid',
    density:'19.1 g/cm³', meltingPoint:'1132.2 °C', boilingPoint:'4131 °C',
    discovered:'1789', discoverer:'Martin Heinrich Klaproth',
    uses:['Nuclear power','Nuclear weapons','Radiation shielding','Dating rocks'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440611'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Uranium'}],
    description:'Heaviest naturally occurring element. Primary fuel for nuclear reactors.' },
{ symbol:'He', name:'Helium', number:2, category:'noble-gas', mass:4.003, phase:'Gas',
    density:'0.0001786 g/cm³', meltingPoint:'-272.20 °C', boilingPoint:'-268.93 °C',
    discovered:'1868', discoverer:'Pierre Janssen & Joseph Lockyer',
    uses:['Balloons','Cryogenics','Welding shield gas','MRI coolant'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440597'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Helium'}],
    description:'Second lightest element. Inert noble gas used in cryogenics and MRI cooling.' },
   { symbol:'Li', name:'Lithium', number:3, category:'alkali-metal', mass:6.941, phase:'Solid',
    density:'0.534 g/cm³', meltingPoint:'180.54 °C', boilingPoint:'1342 °C',
    discovered:'1817', discoverer:'Johan August Arfwedson',
    uses:['Batteries','Ceramics','Pharmaceuticals','Lubricant grease'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439932'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Lithium'}],
    description:'Lightest metal. Key component in rechargeable lithium-ion batteries.' },
   { symbol:'Be', name:'Beryllium', number:4, category:'alkaline-earth', mass:9.012, phase:'Solid',
    density:'1.85 g/cm³', meltingPoint:'1287 °C', boilingPoint:'2470 °C',
    discovered:'1798', discoverer:'Louis Nicolas Vauquelin',
    uses:['Aerospace alloys','X-ray windows','Nuclear reactors','Speakers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440417'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Beryllium'}],
    description:'Lightweight, strong metal. Transparent to X-rays, used in aerospace.' },
   { symbol:'B', name:'Boron', number:5, category:'metalloid', mass:10.81, phase:'Solid',
    density:'2.34 g/cm³', meltingPoint:'2076 °C', boilingPoint:'3927 °C',
    discovered:'1808', discoverer:'Joseph Louis Gay-Lussac & Louis Jacques Thénard',
    uses:['Fiberglass','Borosilicate glass','Detergents','Semiconductors'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440428'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Boron'}],
    description:'Metalloid essential for borosilicate glass and fiberglass insulation.' },
   { symbol:'F', name:'Fluorine', number:9, category:'halogen', mass:18.998, phase:'Gas',
    density:'0.001696 g/cm³', meltingPoint:'-219.67 °C', boilingPoint:'-188.11 °C',
    discovered:'1886', discoverer:'Henri Moissan',
    uses:['Toothpaste','Teflon','Refrigerants','Uranium enrichment'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7782414'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Fluorine'}],
    description:'Most electronegative element. Essential for fluoride in dental care.' },
   { symbol:'Ne', name:'Neon', number:10, category:'noble-gas', mass:20.180, phase:'Gas',
    density:'0.0008999 g/cm³', meltingPoint:'-248.59 °C', boilingPoint:'-246.08 °C',
    discovered:'1898', discoverer:'William Ramsay & Morris Travers',
    uses:['Neon signs','Lasers','Cryogenics','Television tubes'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440019'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Neon'}],
    description:'Noble gas famous for its bright reddish-orange glow in discharge tubes.' },
   { symbol:'Na', name:'Sodium', number:11, category:'alkali-metal', mass:22.990, phase:'Solid',
    density:'0.971 g/cm³', meltingPoint:'97.72 °C', boilingPoint:'883 °C',
    discovered:'1807', discoverer:'Humphry Davy',
    uses:['Table salt','Street lighting','Heat transfer','Chemical synthesis'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440235'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Sodium'}],
    description:'Highly reactive alkali metal. Essential biological element in nerve function.' },
   { symbol:'Mg', name:'Magnesium', number:12, category:'alkaline-earth', mass:24.305, phase:'Solid',
    density:'1.738 g/cm³', meltingPoint:'650 °C', boilingPoint:'1091 °C',
    discovered:'1755', discoverer:'Joseph Black',
    uses:['Lightweight alloys','Fireworks','Antacids','Electronics casings'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439954'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Magnesium'}],
    description:'Light structural metal. Burns with brilliant white flame.' },
   { symbol:'Al', name:'Aluminum', number:13, category:'post-transition-metal', mass:26.982, phase:'Solid',
    density:'2.70 g/cm³', meltingPoint:'660.32 °C', boilingPoint:'2519 °C',
    discovered:'1825', discoverer:'Hans Christian Ørsted',
    uses:['Aircraft','Cans','Foil','Window frames','Electronics'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7429905'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Aluminum'}],
    description:'Most abundant metal in Earth\'s crust. Lightweight and corrosion-resistant.' },
   { symbol:'P', name:'Phosphorus', number:15, category:'nonmetal', mass:30.974, phase:'Solid',
    density:'1.82 g/cm³', meltingPoint:'44.15 °C', boilingPoint:'280.5 °C',
    discovered:'1669', discoverer:'Hennig Brand',
    uses:['Fertilizers','Matches','Detergents','Steel production'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7723140'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Phosphorus'}],
    description:'Essential for life — key part of DNA, RNA, and ATP.' },
   { symbol:'S', name:'Sulfur', number:16, category:'nonmetal', mass:32.06, phase:'Solid',
    density:'2.067 g/cm³', meltingPoint:'115.21 °C', boilingPoint:'444.6 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Sulfuric acid','Fertilizers','Vulcanization','Gunpowder'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7704349'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Sulfur'}],
    description:'Yellow nonmetal known since ancient times. Essential for proteins.' },
   { symbol:'Cl', name:'Chlorine', number:17, category:'halogen', mass:35.45, phase:'Gas',
    density:'0.003214 g/cm³', meltingPoint:'-101.5 °C', boilingPoint:'-34.04 °C',
    discovered:'1774', discoverer:'Carl Wilhelm Scheele',
    uses:['Water treatment','PVC production','Bleach','Disinfectants'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7782505'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Chlorine'}],
    description:'Greenish-yellow halogen. Widely used for water purification.' },
   { symbol:'Ar', name:'Argon', number:18, category:'noble-gas', mass:39.948, phase:'Gas',
    density:'0.001784 g/cm³', meltingPoint:'-189.34 °C', boilingPoint:'-185.85 °C',
    discovered:'1894', discoverer:'Lord Rayleigh & William Ramsay',
    uses:['Welding shield gas','Light bulbs','Insulated windows','Lasers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440371'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Argon'}],
    description:'Third most abundant gas in atmosphere. Inert, used as shielding gas.' },
   { symbol:'K', name:'Potassium', number:19, category:'alkali-metal', mass:39.098, phase:'Solid',
    density:'0.862 g/cm³', meltingPoint:'63.5 °C', boilingPoint:'759 °C',
    discovered:'1807', discoverer:'Humphry Davy',
    uses:['Fertilizers','Soap','Glass','Salt substitutes'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440097'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Potassium'}],
    description:'Essential for plant growth and nerve function. Highly reactive metal.' },
   { symbol:'Ca', name:'Calcium', number:20, category:'alkaline-earth', mass:40.078, phase:'Solid',
    density:'1.55 g/cm³', meltingPoint:'842 °C', boilingPoint:'1484 °C',
    discovered:'1808', discoverer:'Humphry Davy',
    uses:['Cement','Bone health supplements','Steel refining','Cheese making'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440702'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Calcium'}],
    description:'Fifth most abundant element in Earth\'s crust. Essential for bones and teeth.' },
   { symbol:'Sc', name:'Scandium', number:21, category:'transition-metal', mass:44.956, phase:'Solid',
    density:'2.985 g/cm³', meltingPoint:'1541 °C', boilingPoint:'2836 °C',
    discovered:'1879', discoverer:'Lars Fredrik Nilson',
    uses:['Aerospace alloys','Sports equipment','Lighting','Lasers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440200'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Scandium'}],
    description:'Light transition metal that strengthens aluminum alloys for aerospace.' },
   { symbol:'Ti', name:'Titanium', number:22, category:'transition-metal', mass:47.867, phase:'Solid',
    density:'4.506 g/cm³', meltingPoint:'1668 °C', boilingPoint:'3287 °C',
    discovered:'1791', discoverer:'William Gregor',
    uses:['Aerospace','Medical implants','Pigments','Jewelry'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440326'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Titanium'}],
    description:'Strong, lightweight, corrosion-resistant. Ideal for aerospace and implants.' },
   { symbol:'V', name:'Vanadium', number:23, category:'transition-metal', mass:50.942, phase:'Solid',
    density:'6.11 g/cm³', meltingPoint:'1910 °C', boilingPoint:'3407 °C',
    discovered:'1801', discoverer:'Andrés Manuel del Río',
    uses:['Steel alloys','Vanadium redox batteries','Catalysts','Aerospace'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440622'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Vanadium'}],
    description:'Transition metal that adds strength and toughness to steel alloys.' },
   { symbol:'Cr', name:'Chromium', number:24, category:'transition-metal', mass:51.996, phase:'Solid',
    density:'7.15 g/cm³', meltingPoint:'1907 °C', boilingPoint:'2671 °C',
    discovered:'1797', discoverer:'Louis Nicolas Vauquelin',
    uses:['Stainless steel','Chrome plating','Pigments','Leather tanning'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440473'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Chromium'}],
    description:'Hard, lustrous metal. Key ingredient in stainless steel.' },
   { symbol:'Mn', name:'Manganese', number:25, category:'transition-metal', mass:54.938, phase:'Solid',
    density:'7.21 g/cm³', meltingPoint:'1246 °C', boilingPoint:'2061 °C',
    discovered:'1774', discoverer:'Johan Gottlieb Gahn',
    uses:['Steel production','Batteries','Pigments','Water treatment'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439965'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Manganese'}],
    description:'Essential for steel production. Improves strength and hardness.' },
   { symbol:'Co', name:'Cobalt', number:27, category:'transition-metal', mass:58.933, phase:'Solid',
    density:'8.90 g/cm³', meltingPoint:'1495 °C', boilingPoint:'2927 °C',
    discovered:'1735', discoverer:'Georg Brandt',
    uses:['Lithium-ion batteries','Superalloys','Blue pigments','Magnets'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440484'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Cobalt'}],
    description:'Blue-tinted metal critical for rechargeable battery cathodes.' },
   { symbol:'Ni', name:'Nickel', number:28, category:'transition-metal', mass:58.693, phase:'Solid',
    density:'8.908 g/cm³', meltingPoint:'1455 °C', boilingPoint:'2913 °C',
    discovered:'1751', discoverer:'Axel Fredrik Cronstedt',
    uses:['Stainless steel','Coins','Batteries','Plating'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440020'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Nickel'}],
    description:'Corrosion-resistant metal. Major component of stainless steel and coins.' },
   { symbol:'Zn', name:'Zinc', number:30, category:'transition-metal', mass:65.38, phase:'Solid',
    density:'7.134 g/cm³', meltingPoint:'419.53 °C', boilingPoint:'907 °C',
    discovered:'1746', discoverer:'Andreas Sigismund Marggraf',
    uses:['Galvanizing','Alloys (brass)','Batteries','Sunscreen'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440666'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Zinc'}],
    description:'Used to galvanize steel against corrosion. Essential trace element.' },
   { symbol:'Ga', name:'Gallium', number:31, category:'post-transition-metal', mass:69.723, phase:'Solid',
    density:'5.91 g/cm³', meltingPoint:'29.76 °C', boilingPoint:'2204 °C',
    discovered:'1875', discoverer:'Paul Emile Lecoq de Boisbaudran',
    uses:['Semiconductors','LEDs','Solar cells','Thermometers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440553'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Gallium'}],
    description:'Melts near body temperature. Critical for GaAs semiconductors and LEDs.' },
   { symbol:'Ge', name:'Germanium', number:32, category:'metalloid', mass:72.630, phase:'Solid',
    density:'5.323 g/cm³', meltingPoint:'938.25 °C', boilingPoint:'2833 °C',
    discovered:'1886', discoverer:'Clemens Winkler',
    uses:['Fiber optics','Infrared optics','Transistors','Solar cells'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440564'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Germanium'}],
    description:'Semiconductor metalloid. Predicted by Mendeleev as eka-silicon.' },
   { symbol:'As', name:'Arsenic', number:33, category:'metalloid', mass:74.922, phase:'Solid',
    density:'5.776 g/cm³', meltingPoint:'816 °C (sublimes)', boilingPoint:'614 °C (sublimes)',
    discovered:'Ancient', discoverer:'Albertus Magnus (c. 1250)',
    uses:['Semiconductors','Wood preservatives','Pesticides','Lead alloys'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440382'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Arsenic'}],
    description:'Toxic metalloid historically used as a poison. Now used in semiconductors.' },
   { symbol:'Se', name:'Selenium', number:34, category:'nonmetal', mass:78.971, phase:'Solid',
    density:'4.809 g/cm³', meltingPoint:'221 °C', boilingPoint:'685 °C',
    discovered:'1817', discoverer:'Jöns Jacob Berzelius',
    uses:['Glass decolorizer','Electronics','Solar cells','Dietary supplement'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7782492'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Selenium'}],
    description:'Essential trace element. Photoconductor used in photocopiers.' },
   { symbol:'Br', name:'Bromine', number:35, category:'halogen', mass:79.904, phase:'Liquid',
    density:'3.1028 g/cm³', meltingPoint:'-7.2 °C', boilingPoint:'58.8 °C',
    discovered:'1826', discoverer:'Antoine Jérôme Balard',
    uses:['Flame retardants','Pesticides','Pharmaceuticals','Photography'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7726956'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Bromine'}],
    description:'One of two elements liquid at room temperature. Reddish-brown and pungent.' },
   { symbol:'Kr', name:'Krypton', number:36, category:'noble-gas', mass:83.798, phase:'Gas',
    density:'0.003749 g/cm³', meltingPoint:'-157.36 °C', boilingPoint:'-153.22 °C',
    discovered:'1898', discoverer:'William Ramsay & Morris Travers',
    uses:['Lighting','Photography flash','Insulated windows','Lasers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439909'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Krypton'}],
    description:'Noble gas used in high-performance lighting and photographic flash.' },
   { symbol:'Rb', name:'Rubidium', number:37, category:'alkali-metal', mass:85.468, phase:'Solid',
    density:'1.532 g/cm³', meltingPoint:'39.31 °C', boilingPoint:'688 °C',
    discovered:'1861', discoverer:'Robert Bunsen & Gustav Kirchhoff',
    uses:['Atomic clocks','Fireworks','Photocells','Medical imaging'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440177'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Rubidium'}],
    description:'Soft, silvery alkali metal. Used in precision atomic clocks.' },
   { symbol:'Sr', name:'Strontium', number:38, category:'alkaline-earth', mass:87.62, phase:'Solid',
    density:'2.64 g/cm³', meltingPoint:'777 °C', boilingPoint:'1382 °C',
    discovered:'1790', discoverer:'Adair Crawford',
    uses:['Fireworks (red)','Ferrite magnets','Toothpaste','Flares'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440246'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Strontium'}],
    description:'Produces brilliant red flame. Used in fireworks and flares.' },
   { symbol:'Y', name:'Yttrium', number:39, category:'transition-metal', mass:88.906, phase:'Solid',
    density:'4.469 g/cm³', meltingPoint:'1526 °C', boilingPoint:'3345 °C',
    discovered:'1794', discoverer:'Johan Gadolin',
    uses:['LEDs','Superconductors','Lasers','Camera lenses'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440655'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Yttrium'}],
    description:'Silvery metal used in phosphors for LEDs and display screens.' },
   { symbol:'Zr', name:'Zirconium', number:40, category:'transition-metal', mass:91.224, phase:'Solid',
    density:'6.506 g/cm³', meltingPoint:'1855 °C', boilingPoint:'4409 °C',
    discovered:'1789', discoverer:'Martin Heinrich Klaproth',
    uses:['Nuclear fuel cladding','Ceramics','Prosthetics','Chemical processing'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440677'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Zirconium'}],
    description:'Corrosion-resistant metal. Primary use in nuclear reactor fuel cladding.' },
   { symbol:'Nb', name:'Niobium', number:41, category:'transition-metal', mass:92.906, phase:'Solid',
    density:'8.57 g/cm³', meltingPoint:'2477 °C', boilingPoint:'4744 °C',
    discovered:'1801', discoverer:'Charles Hatchett',
    uses:['Superconducting magnets','Steel alloys','Jet engines','Jewelry'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440031'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Niobium'}],
    description:'Superconducting metal used in MRI magnets and particle accelerators.' },
   { symbol:'Mo', name:'Molybdenum', number:42, category:'transition-metal', mass:95.95, phase:'Solid',
    density:'10.22 g/cm³', meltingPoint:'2623 °C', boilingPoint:'4639 °C',
    discovered:'1781', discoverer:'Carl Wilhelm Scheele',
    uses:['Steel alloys','Catalysts','Lubricants','Electronics'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439987'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Molybdenum'}],
    description:'High-melting-point metal that strengthens steel alloys.' },
   { symbol:'Tc', name:'Technetium', number:43, category:'transition-metal', mass:98, phase:'Solid',
    density:'11.5 g/cm³', meltingPoint:'2157 °C', boilingPoint:'4265 °C',
    discovered:'1937', discoverer:'Carlo Perrier & Emilio Segrè',
    uses:['Medical imaging','Nuclear medicine','Radiography','Catalysts'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440262'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Technetium'}],
    description:'First artificially produced element. Widely used in medical diagnostics.' },
   { symbol:'Ru', name:'Ruthenium', number:44, category:'transition-metal', mass:101.07, phase:'Solid',
    density:'12.37 g/cm³', meltingPoint:'2334 °C', boilingPoint:'4150 °C',
    discovered:'1844', discoverer:'Karl Ernst Claus',
    uses:['Catalysts','Electronics','Wear-resistant coatings','Solar cells'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440188'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Ruthenium'}],
    description:'Rare platinum-group metal used in catalysis and electronics.' },
   { symbol:'Rh', name:'Rhodium', number:45, category:'transition-metal', mass:102.906, phase:'Solid',
    density:'12.41 g/cm³', meltingPoint:'1964 °C', boilingPoint:'3695 °C',
    discovered:'1803', discoverer:'William Hyde Wollaston',
    uses:['Catalytic converters','Jewelry','Mirrors','Chemical catalysts'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440166'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Rhodium'}],
    description:'Rarest and most expensive precious metal. Key in automotive catalytic converters.' },
   { symbol:'Pd', name:'Palladium', number:46, category:'transition-metal', mass:106.42, phase:'Solid',
    density:'12.023 g/cm³', meltingPoint:'1554.9 °C', boilingPoint:'2963 °C',
    discovered:'1803', discoverer:'William Hyde Wollaston',
    uses:['Catalytic converters','Electronics','Dentistry','Hydrogen purification'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440053'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Palladium'}],
    description:'Platinum-group metal. Absorbs up to 900 times its own volume of hydrogen.' },
   { symbol:'Cd', name:'Cadmium', number:48, category:'transition-metal', mass:112.414, phase:'Solid',
    density:'8.69 g/cm³', meltingPoint:'321.07 °C', boilingPoint:'767 °C',
    discovered:'1817', discoverer:'Friedrich Stromeyer',
    uses:['Batteries (NiCd)','Pigments','Coatings','Nuclear reactor control rods'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440439'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Cadmium'}],
    description:'Toxic heavy metal. Used in rechargeable batteries and yellow pigments.' },
   { symbol:'In', name:'Indium', number:49, category:'post-transition-metal', mass:114.818, phase:'Solid',
    density:'7.31 g/cm³', meltingPoint:'156.6 °C', boilingPoint:'2072 °C',
    discovered:'1863', discoverer:'Ferdinand Reich & Hieronymus Theodor Richter',
    uses:['Touchscreens (ITO)','Solders','Semiconductors','Low-melting alloys'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440746'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Indium'}],
    description:'Soft metal critical for transparent conductive coatings in touchscreens.' },
   { symbol:'Sn', name:'Tin', number:50, category:'post-transition-metal', mass:118.710, phase:'Solid',
    density:'7.287 g/cm³', meltingPoint:'231.93 °C', boilingPoint:'2602 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Tin cans','Solder','Bronze alloy','Tin plating'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440315'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Tin'}],
    description:'One of the earliest metals used by humans. Key component of bronze and solder.' },
   { symbol:'Sb', name:'Antimony', number:51, category:'metalloid', mass:121.760, phase:'Solid',
    density:'6.685 g/cm³', meltingPoint:'630.63 °C', boilingPoint:'1587 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Flame retardants','Lead-acid batteries','Semiconductors','Cosmetics'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440360'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Antimony'}],
    description:'Brittle metalloid used in flame retardants and lead-acid battery alloys.' },
   { symbol:'Te', name:'Tellurium', number:52, category:'metalloid', mass:127.60, phase:'Solid',
    density:'6.232 g/cm³', meltingPoint:'449.51 °C', boilingPoint:'988 °C',
    discovered:'1783', discoverer:'Franz-Joseph Müller von Reichenstein',
    uses:['Solar cells (CdTe)','Thermoelectrics','Alloys','Rubber vulcanization'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C13494809'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Tellurium'}],
    description:'Rare metalloid used in thin-film solar cells and thermoelectric devices.' },
   { symbol:'I', name:'Iodine', number:53, category:'halogen', mass:126.904, phase:'Solid',
    density:'4.933 g/cm³', meltingPoint:'113.7 °C', boilingPoint:'184.3 °C',
    discovered:'1811', discoverer:'Bernard Courtois',
    uses:['Disinfectants','Thyroid medicine','Photography','Dyes'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7553562'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Iodine'}],
    description:'Essential for thyroid hormones. Purple vapor when heated.' },
   { symbol:'Xe', name:'Xenon', number:54, category:'noble-gas', mass:131.293, phase:'Gas',
    density:'0.005887 g/cm³', meltingPoint:'-111.75 °C', boilingPoint:'-108.10 °C',
    discovered:'1898', discoverer:'William Ramsay & Morris Travers',
    uses:['Headlights','Anesthesia','Ion propulsion','Medical imaging'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440633'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Xenon'}],
    description:'Heavy noble gas. Used in bright arc lamps and spacecraft ion engines.' },
   { symbol:'Cs', name:'Cesium', number:55, category:'alkali-metal', mass:132.905, phase:'Solid',
    density:'1.873 g/cm³', meltingPoint:'28.44 °C', boilingPoint:'671 °C',
    discovered:'1860', discoverer:'Robert Bunsen & Gustav Kirchhoff',
    uses:['Atomic clocks','Drilling fluids','Photoelectric cells','Cancer treatment'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440462'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Cesium'}],
    description:'Most electropositive stable element. Defines the second via atomic clocks.' },
   { symbol:'Ba', name:'Barium', number:56, category:'alkaline-earth', mass:137.327, phase:'Solid',
    density:'3.594 g/cm³', meltingPoint:'727 °C', boilingPoint:'1845 °C',
    discovered:'1808', discoverer:'Humphry Davy',
    uses:['Medical imaging (barium meal)','Drilling fluids','Fireworks (green)','Glass'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440393'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Barium'}],
    description:'Heavy alkaline earth metal. Barium sulfate used in X-ray imaging.' },
   { symbol:'La', name:'Lanthanum', number:57, category:'lanthanide', mass:138.905, phase:'Solid',
    density:'6.145 g/cm³', meltingPoint:'920 °C', boilingPoint:'3464 °C',
    discovered:'1839', discoverer:'Carl Gustaf Mosander',
    uses:['Camera lenses','Catalysts','Lighter flints','Hybrid car batteries'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439910'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Lanthanum'}],
    description:'First of the lanthanides. Used in high-quality optical glass.' },
   { symbol:'Ce', name:'Cerium', number:58, category:'lanthanide', mass:140.116, phase:'Solid',
    density:'6.770 g/cm³', meltingPoint:'799 °C', boilingPoint:'3443 °C',
    discovered:'1803', discoverer:'Jöns Jacob Berzelius & Wilhelm Hisinger',
    uses:['Catalytic converters','Glass polishing','Self-cleaning ovens','Lighter flints'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440451'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Cerium'}],
    description:'Most abundant lanthanide. Used in catalytic converters and glass polishing.' },
   { symbol:'Pr', name:'Praseodymium', number:59, category:'lanthanide', mass:140.908, phase:'Solid',
    density:'6.773 g/cm³', meltingPoint:'931 °C', boilingPoint:'3520 °C',
    discovered:'1885', discoverer:'Carl Auer von Welsbach',
    uses:['Magnets','Aircraft engines','Yellow glass','Lighter flints'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440100'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Praseodymium'}],
    description:'Rare earth used in strong permanent magnets and aircraft engines.' },
   { symbol:'Nd', name:'Neodymium', number:60, category:'lanthanide', mass:144.242, phase:'Solid',
    density:'7.007 g/cm³', meltingPoint:'1021 °C', boilingPoint:'3074 °C',
    discovered:'1885', discoverer:'Carl Auer von Welsbach',
    uses:['Powerful magnets','Lasers','Headphones','Wind turbines'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440008'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Neodymium'}],
    description:'Creates the strongest permanent magnets (NdFeB). Critical for green tech.' },
   { symbol:'Pm', name:'Promethium', number:61, category:'lanthanide', mass:145, phase:'Solid',
    density:'7.26 g/cm³', meltingPoint:'1042 °C', boilingPoint:'3000 °C',
    discovered:'1945', discoverer:'Jacob A. Marinsky, Lawrence E. Glendenin & Charles D. Coryell',
    uses:['Nuclear batteries','Luminous paint','Thickness gauges','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440126'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Promethium'}],
    description:'Only radioactive lanthanide with no stable isotopes. Extremely rare.' },
   { symbol:'Sm', name:'Samarium', number:62, category:'lanthanide', mass:150.36, phase:'Solid',
    density:'7.52 g/cm³', meltingPoint:'1072 °C', boilingPoint:'1794 °C',
    discovered:'1879', discoverer:'Paul Emile Lecoq de Boisbaudran',
    uses:['Magnets','Cancer treatment','Nuclear reactor control','Headphones'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440199'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Samarium'}],
    description:'Used in samarium-cobalt magnets that resist demagnetization at high temperatures.' },
   { symbol:'Eu', name:'Europium', number:63, category:'lanthanide', mass:151.964, phase:'Solid',
    density:'5.243 g/cm³', meltingPoint:'822 °C', boilingPoint:'1529 °C',
    discovered:'1901', discoverer:'Eugène-Anatole Demarçay',
    uses:['Red phosphors (TVs)','Euro banknote security','Lasers','Fluorescent lamps'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440531'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Europium'}],
    description:'Most reactive lanthanide. Provides red color in TV screens and LEDs.' },
   { symbol:'Gd', name:'Gadolinium', number:64, category:'lanthanide', mass:157.25, phase:'Solid',
    density:'7.895 g/cm³', meltingPoint:'1313 °C', boilingPoint:'3273 °C',
    discovered:'1880', discoverer:'Jean Charles Galissard de Marignac',
    uses:['MRI contrast agent','Nuclear reactors','Magnets','Neutron radiography'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440542'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Gadolinium'}],
    description:'Highest neutron absorption of any element. Widely used as MRI contrast agent.' },
   { symbol:'Tb', name:'Terbium', number:65, category:'lanthanide', mass:158.925, phase:'Solid',
    density:'8.229 g/cm³', meltingPoint:'1356 °C', boilingPoint:'3230 °C',
    discovered:'1843', discoverer:'Carl Gustaf Mosander',
    uses:['Green phosphors','Solid-state devices','Sonar systems','Fuel cells'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440279'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Terbium'}],
    description:'Rare earth that provides green color in fluorescent lamps and displays.' },
   { symbol:'Dy', name:'Dysprosium', number:66, category:'lanthanide', mass:162.500, phase:'Solid',
    density:'8.55 g/cm³', meltingPoint:'1412 °C', boilingPoint:'2567 °C',
    discovered:'1886', discoverer:'Paul Emile Lecoq de Boisbaudran',
    uses:['Permanent magnets','Nuclear control rods','Data storage','Lasers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7429916'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Dysprosium'}],
    description:'Added to neodymium magnets to maintain strength at high temperatures.' },
   { symbol:'Ho', name:'Holmium', number:67, category:'lanthanide', mass:164.930, phase:'Solid',
    density:'8.795 g/cm³', meltingPoint:'1474 °C', boilingPoint:'2700 °C',
    discovered:'1878', discoverer:'Marc Delafontaine & Jacques-Louis Soret',
    uses:['Lasers (medical)','Nuclear reactors','Magnets','Spectrophotometry'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440600'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Holmium'}],
    description:'Has the highest magnetic moment of any element. Used in medical lasers.' },
   { symbol:'Er', name:'Erbium', number:68, category:'lanthanide', mass:167.259, phase:'Solid',
    density:'9.066 g/cm³', meltingPoint:'1529 °C', boilingPoint:'2868 °C',
    discovered:'1842', discoverer:'Carl Gustaf Mosander',
    uses:['Fiber optic amplifiers','Lasers','Glass colorant','Nuclear technology'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440520'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Erbium'}],
    description:'Critical for erbium-doped fiber amplifiers that enable long-distance internet.' },
   { symbol:'Tm', name:'Thulium', number:69, category:'lanthanide', mass:168.934, phase:'Solid',
    density:'9.321 g/cm³', meltingPoint:'1545 °C', boilingPoint:'1950 °C',
    discovered:'1879', discoverer:'Per Teodor Cleve',
    uses:['Portable X-ray machines','Lasers','High-temp superconductors','Radiation dosimeters'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440304'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Thulium'}],
    description:'Rarest lanthanide. Used in portable X-ray devices and surgical lasers.' },
   { symbol:'Yb', name:'Ytterbium', number:70, category:'lanthanide', mass:173.045, phase:'Solid',
    density:'6.965 g/cm³', meltingPoint:'819 °C', boilingPoint:'1196 °C',
    discovered:'1878', discoverer:'Jean Charles Galissard de Marignac',
    uses:['Atomic clocks','Stress gauges','Lasers','Metallurgy'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440644'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Ytterbium'}],
    description:'Used in world\'s most precise atomic clocks and as a stress gauge in explosions.' },
   { symbol:'Lu', name:'Lutetium', number:71, category:'lanthanide', mass:174.967, phase:'Solid',
    density:'9.84 g/cm³', meltingPoint:'1663 °C', boilingPoint:'3402 °C',
    discovered:'1907', discoverer:'Georges Urbain',
    uses:['PET scan catalysts','Oil refining','LED phosphors','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439943'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Lutetium'}],
    description:'Hardest and densest lanthanide. Used in PET scan detectors.' },
   { symbol:'Hf', name:'Hafnium', number:72, category:'transition-metal', mass:178.49, phase:'Solid',
    density:'13.31 g/cm³', meltingPoint:'2233 °C', boilingPoint:'4603 °C',
    discovered:'1923', discoverer:'Dirk Coster & George de Hevesy',
    uses:['Nuclear reactor control rods','Superalloys','Plasma cutting','Microprocessors'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440586'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Hafnium'}],
    description:'Excellent neutron absorber. Used in nuclear control rods and Intel processors.' },
   { symbol:'Ta', name:'Tantalum', number:73, category:'transition-metal', mass:180.948, phase:'Solid',
    density:'16.654 g/cm³', meltingPoint:'3017 °C', boilingPoint:'5458 °C',
    discovered:'1802', discoverer:'Anders Gustaf Ekeberg',
    uses:['Capacitors','Surgical implants','Jet engines','Chemical equipment'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440257'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Tantalum'}],
    description:'Highly corrosion-resistant. Essential for capacitors in smartphones.' },
   { symbol:'W', name:'Tungsten', number:74, category:'transition-metal', mass:183.84, phase:'Solid',
    density:'19.25 g/cm³', meltingPoint:'3422 °C', boilingPoint:'5555 °C',
    discovered:'1783', discoverer:'Juan José Elhuyar & Fausto Elhuyar',
    uses:['Light bulb filaments','Cutting tools','Ammunition','Heating elements'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440337'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Tungsten'}],
    description:'Highest melting point of all elements. Extremely hard and dense.' },
   { symbol:'Re', name:'Rhenium', number:75, category:'transition-metal', mass:186.207, phase:'Solid',
    density:'21.02 g/cm³', meltingPoint:'3186 °C', boilingPoint:'5596 °C',
    discovered:'1925', discoverer:'Masataka Ogawa (claimed), Walter Noddack, Ida Tacke & Otto Berg',
    uses:['Jet engine superalloys','Catalysts','Thermocouples','Filaments'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440155'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Rhenium'}],
    description:'One of the rarest elements. Critical for jet engine superalloys.' },
   { symbol:'Os', name:'Osmium', number:76, category:'transition-metal', mass:190.23, phase:'Solid',
    density:'22.587 g/cm³', meltingPoint:'3033 °C', boilingPoint:'5012 °C',
    discovered:'1803', discoverer:'Smithson Tennant',
    uses:['Fountain pen tips','Electrical contacts','Fingerprint detection','Catalysts'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440044'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Osmium'}],
    description:'Densest naturally occurring element. Has a distinctive pungent odor.' },
   { symbol:'Ir', name:'Iridium', number:77, category:'transition-metal', mass:192.217, phase:'Solid',
    density:'22.56 g/cm³', meltingPoint:'2466 °C', boilingPoint:'4428 °C',
    discovered:'1803', discoverer:'Smithson Tennant',
    uses:['Spark plugs','Crucibles','Fountain pen nibs','Kilogram standard'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439885'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Iridium'}],
    description:'Most corrosion-resistant metal. K-T extinction asteroid marker.' },
   { symbol:'Pt', name:'Platinum', number:78, category:'transition-metal', mass:195.084, phase:'Solid',
    density:'21.45 g/cm³', meltingPoint:'1768.3 °C', boilingPoint:'3825 °C',
    discovered:'1735', discoverer:'Antonio de Ulloa',
    uses:['Catalytic converters','Jewelry','Chemotherapy','Fuel cells'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440064'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Platinum'}],
    description:'Precious metal. Critical for catalytic converters and cancer treatment drugs.' },
   { symbol:'Hg', name:'Mercury', number:80, category:'transition-metal', mass:200.592, phase:'Liquid',
    density:'13.534 g/cm³', meltingPoint:'-38.83 °C', boilingPoint:'356.73 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Thermometers','Fluorescent lamps','Dental amalgams','Barometers'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439976'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Mercury'}],
    description:'Only metal that is liquid at room temperature. Toxic heavy metal.' },
   { symbol:'Tl', name:'Thallium', number:81, category:'post-transition-metal', mass:204.38, phase:'Solid',
    density:'11.85 g/cm³', meltingPoint:'304 °C', boilingPoint:'1473 °C',
    discovered:'1861', discoverer:'William Crookes',
    uses:['Electronics','Infrared optics','Rat poison (historic)','Medical imaging'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440280'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Thallium'}],
    description:'Highly toxic heavy metal. Used in infrared optics and electronics.' },
   { symbol:'Pb', name:'Lead', number:82, category:'post-transition-metal', mass:207.2, phase:'Solid',
    density:'11.34 g/cm³', meltingPoint:'327.46 °C', boilingPoint:'1749 °C',
    discovered:'Ancient', discoverer:'Known since antiquity',
    uses:['Batteries','Radiation shielding','Ammunition','Weights'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439921'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Lead'}],
    description:'Dense, soft, toxic metal. Primary use in lead-acid batteries.' },
   { symbol:'Bi', name:'Bismuth', number:83, category:'post-transition-metal', mass:208.980, phase:'Solid',
    density:'9.807 g/cm³', meltingPoint:'271.5 °C', boilingPoint:'1564 °C',
    discovered:'1753', discoverer:'Claude François Geoffroy',
    uses:['Pepto-Bismol','Cosmetics','Lead-free solder','Fire sprinkler alloys'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440699'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Bismuth'}],
    description:'Least toxic heavy metal. Forms beautiful rainbow-colored oxide crystals.' },
   { symbol:'Po', name:'Polonium', number:84, category:'post-transition-metal', mass:209, phase:'Solid',
    density:'9.32 g/cm³', meltingPoint:'254 °C', boilingPoint:'962 °C',
    discovered:'1898', discoverer:'Marie & Pierre Curie',
    uses:['Static eliminators','Nuclear triggers','Heat source (space)','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440086'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Polonium'}],
    description:'Extremely radioactive. Named after Poland by Marie Curie.' },
   { symbol:'At', name:'Astatine', number:85, category:'halogen', mass:210, phase:'Solid',
    density:'~7 g/cm³ (est.)', meltingPoint:'302 °C', boilingPoint:'337 °C (est.)',
    discovered:'1940', discoverer:'Dale R. Corson, Kenneth Ross MacKenzie & Emilio Segrè',
    uses:['Targeted cancer therapy (At-211)','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440681'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Astatine'}],
    description:'Rarest naturally occurring element. Being researched for cancer radiotherapy.' },
   { symbol:'Rn', name:'Radon', number:86, category:'noble-gas', mass:222, phase:'Gas',
    density:'0.00973 g/cm³', meltingPoint:'-71 °C', boilingPoint:'-61.7 °C',
    discovered:'1900', discoverer:'Friedrich Ernst Dorn',
    uses:['Cancer treatment (historic)','Earthquake prediction research','Radon testing'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C10043922'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Radon'}],
    description:'Radioactive noble gas. Second leading cause of lung cancer after smoking.' },
   { symbol:'Fr', name:'Francium', number:87, category:'alkali-metal', mass:223, phase:'Solid',
    density:'~1.87 g/cm³ (est.)', meltingPoint:'27 °C (est.)', boilingPoint:'677 °C (est.)',
    discovered:'1939', discoverer:'Marguerite Perey',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440735'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Francium'}],
    description:'Most unstable naturally occurring element. Half-life of only 22 minutes.' },
   { symbol:'Ra', name:'Radium', number:88, category:'alkaline-earth', mass:226, phase:'Solid',
    density:'5.5 g/cm³', meltingPoint:'696 °C', boilingPoint:'1500 °C',
    discovered:'1898', discoverer:'Marie & Pierre Curie',
    uses:['Cancer treatment (historic)','Luminous paint (historic)','Neutron source','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440144'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Radium'}],
    description:'Intensely radioactive. Once used in glow-in-the-dark watch dials.' },
   { symbol:'Ac', name:'Actinium', number:89, category:'actinide', mass:227, phase:'Solid',
    density:'10.07 g/cm³', meltingPoint:'1050 °C', boilingPoint:'3200 °C',
    discovered:'1899', discoverer:'André-Louis Debierne',
    uses:['Neutron source','Cancer treatment research','Thermoelectric generators'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440341'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Actinium'}],
    description:'First of the actinides. Glows pale blue in the dark due to radioactivity.' },
   { symbol:'Th', name:'Thorium', number:90, category:'actinide', mass:232.038, phase:'Solid',
    density:'11.72 g/cm³', meltingPoint:'1750 °C', boilingPoint:'4788 °C',
    discovered:'1829', discoverer:'Jöns Jacob Berzelius',
    uses:['Nuclear fuel (potential)','Gas mantles','Welding electrodes','Optics'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440291'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Thorium'}],
    description:'Potential nuclear fuel more abundant than uranium. Named after Thor.' },
   { symbol:'Pa', name:'Protactinium', number:91, category:'actinide', mass:231.036, phase:'Solid',
    density:'15.37 g/cm³', meltingPoint:'1572 °C', boilingPoint:'4000 °C',
    discovered:'1913', discoverer:'Kasimir Fajans & Oswald Helmuth Göhring',
    uses:['Research','Ocean sediment dating'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440133'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Protactinium'}],
    description:'Rare, toxic, radioactive actinide. One of the rarest natural elements.' },
   { symbol:'Np', name:'Neptunium', number:93, category:'actinide', mass:237, phase:'Solid',
    density:'20.45 g/cm³', meltingPoint:'644 °C', boilingPoint:'3902 °C',
    discovered:'1940', discoverer:'Edwin McMillan & Philip H. Abelson',
    uses:['Neutron detection','Nuclear waste research','Precursor to Pu-238'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7439998'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Neptunium'}],
    description:'First transuranium element. Named after the planet Neptune.' },
   { symbol:'Pu', name:'Plutonium', number:94, category:'actinide', mass:244, phase:'Solid',
    density:'19.816 g/cm³', meltingPoint:'640 °C', boilingPoint:'3228 °C',
    discovered:'1940', discoverer:'Glenn T. Seaborg, Edwin McMillan, Joseph W. Kennedy & Arthur Wahl',
    uses:['Nuclear weapons','Nuclear power','Space probe RTGs','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440075'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Plutonium'}],
    description:'Used in nuclear weapons and as fuel in space probes. Extremely toxic.' },
   { symbol:'Am', name:'Americium', number:95, category:'actinide', mass:243, phase:'Solid',
    density:'13.69 g/cm³', meltingPoint:'1176 °C', boilingPoint:'2011 °C',
    discovered:'1944', discoverer:'Glenn T. Seaborg, Ralph A. James, Leon O. Morgan & Albert Ghiorso',
    uses:['Smoke detectors','Neutron sources','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440352'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Americium'}],
    description:'Found in household smoke detectors. Named after the Americas.' },
   { symbol:'Cm', name:'Curium', number:96, category:'actinide', mass:247, phase:'Solid',
    density:'13.51 g/cm³', meltingPoint:'1345 °C', boilingPoint:'3110 °C',
    discovered:'1944', discoverer:'Glenn T. Seaborg, Ralph A. James & Albert Ghiorso',
    uses:['Space probe power (RTGs)','Alpha particle source','Research'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440510'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Curium'}],
    description:'Named after Marie & Pierre Curie. Powers some Mars rovers.' },
   { symbol:'Bk', name:'Berkelium', number:97, category:'actinide', mass:247, phase:'Solid',
    density:'14.78 g/cm³', meltingPoint:'986 °C', boilingPoint:'2627 °C',
    discovered:'1949', discoverer:'Glenn T. Seaborg, Stanley G. Thompson & Albert Ghiorso',
    uses:['Research','Target for heavier element synthesis'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440405'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Berkelium'}],
    description:'Named after Berkeley, California. Only produced in microgram quantities.' },
   { symbol:'Cf', name:'Californium', number:98, category:'actinide', mass:251, phase:'Solid',
    density:'15.1 g/cm³', meltingPoint:'900 °C', boilingPoint:'1472 °C',
    discovered:'1950', discoverer:'Glenn T. Seaborg, Stanley G. Thompson, Albert Ghiorso & Kenneth Street Jr.',
    uses:['Neutron source','Mineral analysis','Nuclear reactor startup','Cancer treatment'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440717'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Californium'}],
    description:'Strong neutron emitter. Used to detect gold and water in oil wells.' },
   { symbol:'Es', name:'Einsteinium', number:99, category:'actinide', mass:252, phase:'Solid',
    density:'8.84 g/cm³', meltingPoint:'860 °C', boilingPoint:'996 °C (est.)',
    discovered:'1952', discoverer:'Albert Ghiorso et al.',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7429926'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Einsteinium'}],
    description:'Named after Albert Einstein. Discovered in debris of first hydrogen bomb test.' },
   { symbol:'Fm', name:'Fermium', number:100, category:'actinide', mass:257, phase:'Solid',
    density:'~9.7 g/cm³ (est.)', meltingPoint:'1527 °C (est.)', boilingPoint:'Unknown',
    discovered:'1952', discoverer:'Albert Ghiorso et al.',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440726'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Fermium'}],
    description:'Named after Enrico Fermi. Can only be produced in nuclear reactors.' },
   { symbol:'Md', name:'Mendelevium', number:101, category:'actinide', mass:258, phase:'Solid',
    density:'~10.3 g/cm³ (est.)', meltingPoint:'827 °C (est.)', boilingPoint:'Unknown',
    discovered:'1955', discoverer:'Albert Ghiorso, Bernard G. Harvey, Gregory R. Choppin, Stanley G. Thompson & Glenn T. Seaborg',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C7440111'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Mendelevium'}],
    description:'Named after Dmitri Mendeleev. Only about 17 atoms produced at a time.' },
   { symbol:'No', name:'Nobelium', number:102, category:'actinide', mass:259, phase:'Solid',
    density:'~9.9 g/cm³ (est.)', meltingPoint:'827 °C (est.)', boilingPoint:'Unknown',
    discovered:'1958', discoverer:'Albert Ghiorso, Torbjørn Sikkeland, John R. Walton & Glenn T. Seaborg',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C10028148'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Nobelium'}],
    description:'Named after Alfred Nobel. Most stable isotope has half-life of 58 minutes.' },
   { symbol:'Lr', name:'Lawrencium', number:103, category:'actinide', mass:266, phase:'Solid',
    density:'~14.4 g/cm³ (est.)', meltingPoint:'1627 °C (est.)', boilingPoint:'Unknown',
    discovered:'1961', discoverer:'Albert Ghiorso, Torbjørn Sikkeland, Almon Larsh & Robert M. Latimer',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C22537191'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Lawrencium'}],
    description:'Last actinide. Named after Ernest O. Lawrence, inventor of the cyclotron.' },
   { symbol:'Rf', name:'Rutherfordium', number:104, category:'transition-metal', mass:267, phase:'Solid',
    density:'~23.2 g/cm³ (est.)', meltingPoint:'~2100 °C (est.)', boilingPoint:'~5500 °C (est.)',
    discovered:'1964', discoverer:'Joint Institute for Nuclear Research (Dubna)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C53850367'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Rutherfordium'}],
    description:'First transactinide. Named after Ernest Rutherford.' },
   { symbol:'Db', name:'Dubnium', number:105, category:'transition-metal', mass:268, phase:'Solid',
    density:'~29.3 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1967', discoverer:'Joint Institute for Nuclear Research (Dubna)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C53850423'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Dubnium'}],
    description:'Named after Dubna, Russia. Most stable isotope lasts about 28 hours.' },
   { symbol:'Sg', name:'Seaborgium', number:106, category:'transition-metal', mass:269, phase:'Solid',
    density:'~35 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1974', discoverer:'Albert Ghiorso et al.',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54038818'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Seaborgium'}],
    description:'Named after Glenn T. Seaborg while he was still alive — a first.' },
   { symbol:'Bh', name:'Bohrium', number:107, category:'transition-metal', mass:270, phase:'Solid',
    density:'~37.1 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1981', discoverer:'Peter Armbruster & Gottfried Münzenberg',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54037149'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Bohrium'}],
    description:'Named after Niels Bohr. Only a few atoms have ever been produced.' },
   { symbol:'Hs', name:'Hassium', number:108, category:'transition-metal', mass:277, phase:'Solid',
    density:'~40.7 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1984', discoverer:'Peter Armbruster & Gottfried Münzenberg',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54037577'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Hassium'}],
    description:'Named after Hesse, Germany. Behaves like osmium chemically.' },
   { symbol:'Mt', name:'Meitnerium', number:109, category:'transition-metal', mass:278, phase:'Solid',
    density:'~37.4 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1982', discoverer:'Peter Armbruster & Gottfried Münzenberg',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54038016'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Meitnerium'}],
    description:'Named after Lise Meitner, who helped discover nuclear fission.' },
   { symbol:'Ds', name:'Darmstadtium', number:110, category:'transition-metal', mass:281, phase:'Solid',
    density:'~34.8 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1994', discoverer:'Sigurd Hofmann et al. (GSI Darmstadt)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54083778'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Darmstadtium'}],
    description:'Named after Darmstadt, Germany. Extremely short-lived.' },
   { symbol:'Rg', name:'Roentgenium', number:111, category:'transition-metal', mass:282, phase:'Solid',
    density:'~28.7 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'Unknown',
    discovered:'1994', discoverer:'Sigurd Hofmann et al. (GSI Darmstadt)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54386243'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Roentgenium'}],
    description:'Named after Wilhelm Röntgen, discoverer of X-rays.' },
   { symbol:'Cn', name:'Copernicium', number:112, category:'transition-metal', mass:285, phase:'Liquid (predicted)',
    density:'~23.7 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'~84 °C (est.)',
    discovered:'1996', discoverer:'Sigurd Hofmann et al. (GSI Darmstadt)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54084269'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Copernicium'}],
    description:'Named after Nicolaus Copernicus. May be liquid at room temperature.' },
   { symbol:'Nh', name:'Nihonium', number:113, category:'post-transition-metal', mass:286, phase:'Solid (predicted)',
    density:'~16 g/cm³ (est.)', meltingPoint:'~430 °C (est.)', boilingPoint:'~1100 °C (est.)',
    discovered:'2003', discoverer:'RIKEN (Kosuke Morita et al.)',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C71759'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Nihonium'}],
    description:'First element discovered in Asia. Named after Japan (Nihon).' },
   { symbol:'Fl', name:'Flerovium', number:114, category:'post-transition-metal', mass:289, phase:'Solid (predicted)',
    density:'~14 g/cm³ (est.)', meltingPoint:'~67 °C (est.)', boilingPoint:'~147 °C (est.)',
    discovered:'1998', discoverer:'Joint Institute for Nuclear Research (Dubna) & LLNL',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54085161'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Flerovium'}],
    description:'Named after Flerov Laboratory. May behave like a noble gas.' },
   { symbol:'Mc', name:'Moscovium', number:115, category:'post-transition-metal', mass:290, phase:'Solid (predicted)',
    density:'~13.5 g/cm³ (est.)', meltingPoint:'~400 °C (est.)', boilingPoint:'~1100 °C (est.)',
    discovered:'2003', discoverer:'Joint Institute for Nuclear Research (Dubna) & LLNL',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C71750'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Moscovium'}],
    description:'Named after Moscow Oblast. Superheavy element with very short half-life.' },
   { symbol:'Lv', name:'Livermorium', number:116, category:'post-transition-metal', mass:293, phase:'Solid (predicted)',
    density:'~12.9 g/cm³ (est.)', meltingPoint:'~364 °C (est.)', boilingPoint:'~762 °C (est.)',
    discovered:'2000', discoverer:'Joint Institute for Nuclear Research (Dubna) & LLNL',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54100710'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Livermorium'}],
    description:'Named after Lawrence Livermore National Laboratory.' },
   { symbol:'Ts', name:'Tennessine', number:117, category:'halogen', mass:294, phase:'Solid (predicted)',
    density:'~7.2 g/cm³ (est.)', meltingPoint:'~350 °C (est.)', boilingPoint:'~610 °C (est.)',
    discovered:'2010', discoverer:'Joint Institute for Nuclear Research (Dubna) & ORNL',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C87658'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Tennessine'}],
    description:'Named after Tennessee. Second heaviest known halogen.' },
   { symbol:'Og', name:'Oganesson', number:118, category:'noble-gas', mass:294, phase:'Solid (predicted)',
    density:'~5.0 g/cm³ (est.)', meltingPoint:'Unknown', boilingPoint:'~80 °C (est.)',
    discovered:'2002', discoverer:'Joint Institute for Nuclear Research (Dubna) & LLNL',
    uses:['Research only'],
    sources:[{name:'NIST',url:'https://webbook.nist.gov/cgi/cbook.cgi?ID=C54144196'},{name:'PubChem',url:'https://pubchem.ncbi.nlm.nih.gov/element/Oganesson'}],
    description:'Heaviest known element. Named after Yuri Oganessian. May be solid, not gas.' },
  ];

  // ══════════════════════════════════════
  // WORLD PRODUCTION DATA (USGS 2024 / World Bank 2024)
  // ══════════════════════════════════════
  const ELEMENT_WORLD_DATA = {
   H: { production:{annual:'100 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Virtually unlimited (water)',yearsSupply:null}, topProducers:['China','USA','EU','Middle East'], price:{amount:1500,unit:'$/tonne',year:2024}, primaryUses:['Ammonia/fertilizer (55%)','Oil refining (25%)','Methanol (10%)','Fuel cells'] },
   He: { production:{annual:'160 million m³',unit:'m³/yr'}, reserves:{amount:'31.3 billion m³',yearsSupply:195}, topProducers:['USA','Qatar','Algeria','Russia','Poland'], price:{amount:35,unit:'$/m³',year:2024}, primaryUses:['Cryogenics/MRI (30%)','Welding (17%)','Leak detection','Balloons'] },
   Li: { production:{annual:'130,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'28 million tonnes',yearsSupply:215}, topProducers:['Australia','Chile','China','Argentina'], price:{amount:17000,unit:'$/tonne',year:2024}, primaryUses:['Batteries (80%)','Ceramics/glass (7%)','Lubricant grease','Pharmaceuticals'] },
   Be: { production:{annual:'260 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Undisclosed',yearsSupply:null}, topProducers:['USA','China','Mozambique','Brazil'], price:{amount:857000,unit:'$/tonne',year:2024}, primaryUses:['Aerospace alloys','Electronics','X-ray windows','Nuclear reactors'] },
   B: { production:{annual:'4.4 million tonnes (boron oxide)',unit:'tonnes/yr'}, reserves:{amount:'1.2 billion tonnes',yearsSupply:270}, topProducers:['Turkey (68%)','USA','Chile','Russia'], price:{amount:450,unit:'$/tonne (boric acid)',year:2024}, primaryUses:['Glass/fiberglass (46%)','Detergents (12%)','Agriculture','Ceramics'] },
   C: { production:{annual:'130 million carats industrial diamond; 1.1B tonnes coal',unit:'mixed'}, reserves:{amount:'1.07 trillion tonnes coal',yearsSupply:139}, topProducers:['China','India','Indonesia','USA','Australia'], price:{amount:null,unit:'varies widely',year:2024}, primaryUses:['Steel production','Fuels','Plastics/chemicals','Diamond tools'] },
   N: { production:{annual:'150 million tonnes (as fertilizer N)',unit:'tonnes/yr'}, reserves:{amount:'Unlimited (78% of atmosphere)',yearsSupply:null}, topProducers:['China','India','USA','EU','Russia'], price:{amount:400,unit:'$/tonne (ammonia)',year:2024}, primaryUses:['Fertilizer (80%)','Explosives','Nylon/plastics','Cryogenics'] },
   O: { production:{annual:'360 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Unlimited (atmosphere + water)',yearsSupply:null}, topProducers:['China','USA','EU','India','Japan'], price:{amount:100,unit:'$/tonne (liquid)',year:2024}, primaryUses:['Steel manufacturing (55%)','Medical','Welding','Chemical synthesis'] },
   F: { production:{annual:'8.3 million tonnes (fluorspar)',unit:'tonnes/yr'}, reserves:{amount:'310 million tonnes',yearsSupply:37}, topProducers:['China (64%)','Mexico','Mongolia','South Africa'], price:{amount:560,unit:'$/tonne (fluorspar)',year:2024}, primaryUses:['Aluminum smelting','Refrigerants/PTFE','Uranium enrichment','Toothpaste'] },
   Ne: { production:{note:'Byproduct of air separation — ~70,000 tonnes/yr'}, topProducers:['Ukraine','China','USA','Japan'], price:{amount:120,unit:'$/m³',year:2024}, primaryUses:['Semiconductor lithography','Neon signs','Lasers','Cryogenics'] },
   Na: { production:{annual:'280 million tonnes (as NaCl salt)',unit:'tonnes/yr'}, reserves:{amount:'Virtually unlimited',yearsSupply:null}, topProducers:['China','USA','India','Germany','Australia'], price:{amount:40,unit:'$/tonne (salt)',year:2024}, primaryUses:['Food/seasoning (35%)','Chemical industry (35%)','De-icing','Water treatment'] },
   Mg: { production:{annual:'1.1 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Virtually unlimited (seawater)',yearsSupply:null}, topProducers:['China (90%)','USA','Israel','Brazil','Turkey'], price:{amount:2800,unit:'$/tonne',year:2024}, primaryUses:['Aluminum alloys (42%)','Die casting','Steel desulfurization','Aerospace'] },
   Al: { production:{annual:'69 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'55-75 billion tonnes (bauxite)',yearsSupply:100}, topProducers:['China (57%)','India','Russia','Canada','UAE'], price:{amount:2400,unit:'$/tonne',year:2024}, primaryUses:['Transportation (27%)','Packaging (20%)','Construction (20%)','Electrical'] },
   Si: { production:{annual:'8.8 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Abundant (28% of crust)',yearsSupply:null}, topProducers:['China (70%)','Russia','Brazil','Norway','France'], price:{amount:2500,unit:'$/tonne',year:2024}, primaryUses:['Aluminum alloys (40%)','Silicones (30%)','Semiconductors','Solar cells'] },
   P: { production:{annual:'220 million tonnes (phosphate rock)',unit:'tonnes/yr'}, reserves:{amount:'72 billion tonnes',yearsSupply:327}, topProducers:['China (40%)','Morocco','USA','Russia','Brazil'], price:{amount:100,unit:'$/tonne (rock)',year:2024}, primaryUses:['Fertilizer (85%)','Animal feed','Detergents','Food additives'] },
   S: { production:{annual:'80 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Abundant (byproduct)',yearsSupply:null}, topProducers:['China','USA','Russia','Canada','Saudi Arabia'], price:{amount:80,unit:'$/tonne',year:2024}, primaryUses:['Sulfuric acid (60%)','Fertilizer','Rubber vulcanization','Chemicals'] },
   Cl: { production:{annual:'75 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'Virtually unlimited (sea salt)',yearsSupply:null}, topProducers:['China','USA','EU','India','Japan'], price:{amount:200,unit:'$/tonne',year:2024}, primaryUses:['PVC (35%)','Water treatment','Solvents','Bleach/disinfection'] },
   Ar: { production:{annual:'700,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Unlimited (0.93% of atmosphere)',yearsSupply:null}, topProducers:['China','USA','EU','Japan','India'], price:{amount:3,unit:'$/m³',year:2024}, primaryUses:['Welding shield gas (55%)','Lighting','Insulated windows','Semiconductor fab'] },
   K: { production:{annual:'45 million tonnes (K₂O equiv)',unit:'tonnes/yr'}, reserves:{amount:'3.7 billion tonnes',yearsSupply:82}, topProducers:['Canada (30%)','Russia','Belarus','China','Germany'], price:{amount:300,unit:'$/tonne (KCl)',year:2024}, primaryUses:['Fertilizer (95%)','Soap/glass','Salt substitutes','Industrial chemicals'] },
   Ca: { production:{annual:'350 million tonnes (as lime/limestone)',unit:'tonnes/yr'}, reserves:{amount:'Virtually unlimited',yearsSupply:null}, topProducers:['China','USA','India','Russia','Japan'], price:{amount:100,unit:'$/tonne (lime)',year:2024}, primaryUses:['Cement (70%)','Steel flux','Agriculture','Water treatment'] },
   Sc: { production:{annual:'15-20 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Unknown (scattered deposits)',yearsSupply:null}, topProducers:['China','Russia','Philippines','Ukraine'], price:{amount:3600,unit:'$/kg',year:2024}, primaryUses:['Aluminum-scandium alloys','Solid oxide fuel cells','Sports equipment','Lighting'] },
   Ti: { production:{annual:'7.5 million tonnes (TiO₂ concentrate)',unit:'tonnes/yr'}, reserves:{amount:'700 million tonnes',yearsSupply:93}, topProducers:['China','Mozambique','South Africa','Australia','Canada'], price:{amount:11000,unit:'$/tonne (sponge)',year:2024}, primaryUses:['Pigments/TiO₂ (93%)','Aerospace metal (4%)','Medical implants','Welding rod coatings'] },
   V: { production:{annual:'110,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'24 million tonnes',yearsSupply:218}, topProducers:['China (66%)','Russia','South Africa','Brazil'], price:{amount:28,unit:'$/kg',year:2024}, primaryUses:['Steel alloys (85%)','Titanium alloys','Vanadium redox batteries','Catalysts'] },
   Cr: { production:{annual:'41 million tonnes (ore)',unit:'tonnes/yr'}, reserves:{amount:'570 million tonnes',yearsSupply:14}, topProducers:['South Africa (44%)','Turkey','Kazakhstan','India','Finland'], price:{amount:10000,unit:'$/tonne (ferrochromium)',year:2024}, primaryUses:['Stainless steel (70%)','Chrome plating','Pigments','Refractories'] },
   Mn: { production:{annual:'20 million tonnes (ore)',unit:'tonnes/yr'}, reserves:{amount:'1.7 billion tonnes',yearsSupply:85}, topProducers:['South Africa (37%)','Gabon','Australia','Ghana','China'], price:{amount:1500,unit:'$/tonne (ore)',year:2024}, primaryUses:['Steel production (90%)','Batteries','Aluminum alloys','Chemicals'] },
   Fe: { production:{annual:'2.5 billion tonnes (ore)',unit:'tonnes/yr'}, reserves:{amount:'170 billion tonnes',yearsSupply:68}, topProducers:['Australia','Brazil','China','India','Russia'], price:{amount:120,unit:'$/tonne',year:2024}, primaryUses:['Steel production (98%)','Cast iron','Magnets','Pigments'] },
   Co: { production:{annual:'190,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'8.3 million tonnes',yearsSupply:44}, topProducers:['DRC (73%)','Indonesia','Russia','Australia','Philippines'], price:{amount:30000,unit:'$/tonne',year:2024}, primaryUses:['Batteries (46%)','Superalloys (19%)','Catalysts','Magnets/pigments'] },
   Ni: { production:{annual:'3.6 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'110 million tonnes',yearsSupply:31}, topProducers:['Indonesia (49%)','Philippines','Russia','New Caledonia','Australia'], price:{amount:16000,unit:'$/tonne',year:2024}, primaryUses:['Stainless steel (65%)','Batteries (15%)','Alloys','Plating'] },
   Cu: { production:{annual:'22 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'890 million tonnes',yearsSupply:40}, topProducers:['Chile (24%)','DRC','Peru','China','Indonesia'], price:{amount:8500,unit:'$/tonne',year:2024}, primaryUses:['Electrical wire (60%)','Construction (20%)','Transportation','Electronics'] },
   Zn: { production:{annual:'13 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'210 million tonnes',yearsSupply:16}, topProducers:['China (33%)','Australia','Peru','India','USA'], price:{amount:2700,unit:'$/tonne',year:2024}, primaryUses:['Galvanizing (50%)','Alloys/brass (17%)','Chemicals','Batteries'] },
   Ga: { production:{annual:'550 tonnes',unit:'tonnes/yr'}, reserves:{amount:'110,000 tonnes',yearsSupply:200}, topProducers:['China (98%)','Japan','South Korea','Russia'], price:{amount:300,unit:'$/kg',year:2024}, primaryUses:['Integrated circuits (43%)','LEDs/optoelectronics (40%)','Solar cells','Research'] },
   Ge: { production:{annual:'140 tonnes',unit:'tonnes/yr'}, reserves:{amount:'8,600 tonnes',yearsSupply:61}, topProducers:['China (68%)','Russia','USA','Belgium'], price:{amount:1800,unit:'$/kg',year:2024}, primaryUses:['Fiber optics (35%)','Infrared optics (25%)','PET catalysts','Solar cells'] },
   As: { production:{annual:'35,000 tonnes (arsenic trioxide)',unit:'tonnes/yr'}, reserves:{amount:'Moderate',yearsSupply:null}, topProducers:['China (50%)','Morocco','Russia','Belgium'], price:{amount:1,unit:'$/kg (trioxide)',year:2024}, primaryUses:['Wood preservatives (50%)','Semiconductors (GaAs)','Lead alloys','Pesticides'] },
   Se: { production:{annual:'2,800 tonnes',unit:'tonnes/yr'}, reserves:{amount:'100,000 tonnes',yearsSupply:36}, topProducers:['China','Japan','Belgium','Germany','Russia'], price:{amount:55,unit:'$/kg',year:2024}, primaryUses:['Glass manufacturing (30%)','Electronics','Solar cells (CdTe)','Metallurgy'] },
   Br: { production:{annual:'500,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Abundant (seawater)',yearsSupply:null}, topProducers:['China','USA','Israel','Jordan'], price:{amount:2,unit:'$/kg',year:2024}, primaryUses:['Flame retardants (50%)','Drilling fluids','Pesticides','Pharmaceuticals'] },
   Kr: { production:{note:'Byproduct of air separation — ~10 tonnes/yr'}, topProducers:['USA','EU','Japan','China'], price:{amount:500,unit:'$/m³',year:2024}, primaryUses:['Insulated windows','Photography flash','Fluorescent lighting','Lasers'] },
   Rb: { production:{note:'No dedicated mining — byproduct of lithium/cesium production'}, topProducers:['Canada','Namibia','Zambia'], price:{amount:12000,unit:'$/kg',year:2024}, primaryUses:['Atomic clocks','Photocells','Specialty glass','Medical imaging'] },
   Sr: { production:{annual:'210,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'6.8 million tonnes',yearsSupply:32}, topProducers:['Spain','Mexico','China','Iran','Turkey'], price:{amount:700,unit:'$/tonne (celestite)',year:2024}, primaryUses:['Ferrite magnets (55%)','Pyrotechnics','Zinc smelting','Drilling fluids'] },
   Y: { production:{annual:'8,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'540,000 tonnes',yearsSupply:68}, topProducers:['China (93%)','Australia','Myanmar','India'], price:{amount:12,unit:'$/kg (oxide)',year:2024}, primaryUses:['Ceramics/phosphors','Superconductors','Lasers','Alloy additive'] },
   Zr: { production:{annual:'1.4 million tonnes (zircon)',unit:'tonnes/yr'}, reserves:{amount:'67 million tonnes',yearsSupply:48}, topProducers:['Australia','South Africa','Mozambique','China','Indonesia'], price:{amount:2200,unit:'$/tonne (zircon)',year:2024}, primaryUses:['Ceramics (54%)','Nuclear fuel cladding','Refractories','Foundry sands'] },
   Nb: { production:{annual:'81,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'17 million tonnes',yearsSupply:210}, topProducers:['Brazil (90%)','Canada','Australia','DRC'], price:{amount:42,unit:'$/kg',year:2024}, primaryUses:['HSLA steel (85%)','Superconducting magnets','Superalloys','Jewelry'] },
   Mo: { production:{annual:'300,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'18 million tonnes',yearsSupply:60}, topProducers:['China (44%)','Chile','USA','Peru','Mexico'], price:{amount:45,unit:'$/kg',year:2024}, primaryUses:['Steel alloys (70%)','Catalysts','Lubricants','Chemicals'] },
   Tc: { production:{note:'Synthetic — produced in nuclear reactors, ~150 kg/yr for medical use'}, primaryUses:['Medical imaging (Tc-99m)','Industrial radiography'] },
   Ru: { production:{annual:'38 tonnes',unit:'tonnes/yr'}, reserves:{amount:'5,000 tonnes',yearsSupply:132}, topProducers:['South Africa (93%)','Zimbabwe','Russia','Canada'], price:{amount:14000,unit:'$/kg',year:2024}, primaryUses:['Electronics (42%)','Catalysts','Wear-resistant coatings','Electrochemistry'] },
   Rh: { production:{annual:'32 tonnes',unit:'tonnes/yr'}, reserves:{amount:'3,000 tonnes',yearsSupply:94}, topProducers:['South Africa (81%)','Russia','Zimbabwe','Canada'], price:{amount:145000,unit:'$/kg',year:2024}, primaryUses:['Catalytic converters (80%)','Chemical catalysts','Jewelry','Glass production'] },
   Pd: { production:{annual:'210 tonnes',unit:'tonnes/yr'}, reserves:{amount:'9,000 tonnes',yearsSupply:43}, topProducers:['Russia (40%)','South Africa (36%)','Canada','USA','Zimbabwe'], price:{amount:35000,unit:'$/kg',year:2024}, primaryUses:['Catalytic converters (80%)','Electronics','Dentistry','Hydrogen purification'] },
   Ag: { production:{annual:'26,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'530,000 tonnes',yearsSupply:20}, topProducers:['Mexico','China','Peru','Chile','Poland'], price:{amount:800,unit:'$/kg',year:2024}, primaryUses:['Electronics (30%)','Solar panels (17%)','Jewelry/silverware','Photography'] },
   Cd: { production:{annual:'23,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'500,000 tonnes',yearsSupply:22}, topProducers:['China (36%)','South Korea','Japan','Canada','Kazakhstan'], price:{amount:3,unit:'$/kg',year:2024}, primaryUses:['NiCd batteries (72%)','Pigments','Coatings','Solar cells (CdTe)'] },
   In: { production:{annual:'920 tonnes',unit:'tonnes/yr'}, reserves:{amount:'18,000 tonnes',yearsSupply:20}, topProducers:['China (56%)','South Korea','Japan','Canada','France'], price:{amount:280,unit:'$/kg',year:2024}, primaryUses:['ITO for displays (56%)','Solders','Semiconductors','Photovoltaics'] },
   Sn: { production:{annual:'310,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'4.6 million tonnes',yearsSupply:15}, topProducers:['China (30%)','Indonesia','Myanmar','Peru','DRC'], price:{amount:25000,unit:'$/tonne',year:2024}, primaryUses:['Solder (48%)','Tin plating (14%)','Chemicals','Alloys (bronze/pewter)'] },
   Sb: { production:{annual:'160,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'1.8 million tonnes',yearsSupply:11}, topProducers:['China (55%)','Tajikistan','Russia','Myanmar','Bolivia'], price:{amount:12,unit:'$/kg',year:2024}, primaryUses:['Flame retardants (60%)','Lead-acid batteries','Plastics catalysts','Ammunition'] },
   Te: { production:{annual:'580 tonnes',unit:'tonnes/yr'}, reserves:{amount:'31,000 tonnes',yearsSupply:53}, topProducers:['China','Japan','Canada','Sweden','USA'], price:{amount:70,unit:'$/kg',year:2024}, primaryUses:['Solar cells CdTe (40%)','Thermoelectrics','Steel/copper alloys','Rubber vulcanization'] },
   I: { production:{annual:'32,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'6 million tonnes',yearsSupply:188}, topProducers:['Chile (57%)','Japan (25%)','USA','Russia','Turkmenistan'], price:{amount:35,unit:'$/kg',year:2024}, primaryUses:['X-ray contrast media','Biocides','Nylon production','Pharmaceuticals'] },
   Xe: { production:{note:'Byproduct of air separation — ~40 tonnes/yr'}, topProducers:['USA','EU','Japan','China'], price:{amount:3000,unit:'$/m³',year:2024}, primaryUses:['Anesthesia','Ion propulsion','Lighting','Medical imaging'] },
   Cs: { production:{annual:'30 tonnes',unit:'tonnes/yr'}, reserves:{amount:'170,000 tonnes',yearsSupply:5667}, topProducers:['Australia','Canada','Zimbabwe','Namibia'], price:{amount:78000,unit:'$/kg',year:2024}, primaryUses:['Drilling fluids (50%)','Atomic clocks','Photoelectric cells','Cancer treatment'] },
   Ba: { production:{annual:'7.5 million tonnes (barite)',unit:'tonnes/yr'}, reserves:{amount:'300 million tonnes',yearsSupply:40}, topProducers:['China (40%)','India','Morocco','Mexico','Turkey'], price:{amount:160,unit:'$/tonne (barite)',year:2024}, primaryUses:['Oil drilling fluids (80%)','Medical imaging','Rubber filler','Glass'] },
   La: { production:{note:'Included in rare earth totals — ~60,000 tonnes/yr as oxide'}, topProducers:['China (70%)','Myanmar','Australia','USA'], primaryUses:['Fluid catalytic cracking (30%)','Battery alloys','Optical glass','Hybrid car batteries'] },
   Ce: { production:{note:'Most abundant rare earth — ~80,000 tonnes/yr as oxide'}, topProducers:['China (70%)','Myanmar','Australia','USA'], primaryUses:['Catalytic converters','Glass polishing','Self-cleaning ovens','Metallurgy'] },
   Pr: { production:{note:'Rare earth — ~10,000 tonnes/yr as oxide'}, topProducers:['China (70%)','Myanmar','Australia'], primaryUses:['NdFeB magnets (co-constituent)','Aircraft engines','Ceramics','Glass colorant'] },
   Nd: { production:{note:'Rare earth — ~40,000 tonnes/yr as oxide'}, topProducers:['China (70%)','Myanmar','Australia','USA'], price:{amount:80,unit:'$/kg (oxide)',year:2024}, primaryUses:['NdFeB permanent magnets (87%)','Lasers','Glass colorant','Capacitors'] },
   Pm: { production:{note:'Synthetic — trace amounts from nuclear reactors'}, primaryUses:['Nuclear batteries','Research','Luminous paint'] },
   Sm: { production:{note:'Rare earth — ~2,000 tonnes/yr'}, topProducers:['China','Myanmar','Australia'], price:{amount:4,unit:'$/kg (oxide)',year:2024}, primaryUses:['Samarium-cobalt magnets','Cancer treatment','Nuclear control rods','Catalysts'] },
   Eu: { production:{note:'Rare earth — ~400 tonnes/yr'}, topProducers:['China (90%)','Myanmar'], price:{amount:30,unit:'$/kg (oxide)',year:2024}, primaryUses:['Red phosphors/LEDs','Euro banknote security','Fluorescent lamps','Nuclear control rods'] },
   Gd: { production:{note:'Rare earth — ~4,000 tonnes/yr'}, topProducers:['China','Myanmar','Australia'], price:{amount:35,unit:'$/kg (oxide)',year:2024}, primaryUses:['MRI contrast agents','Neutron shielding','Magnets','Phosphors'] },
   Tb: { production:{note:'Rare earth — ~400 tonnes/yr'}, topProducers:['China (90%)','Myanmar'], price:{amount:1200,unit:'$/kg (oxide)',year:2024}, primaryUses:['NdFeB magnet additive','Green phosphors','Sonar','Fuel cells'] },
   Dy: { production:{note:'Rare earth — ~2,500 tonnes/yr'}, topProducers:['China (85%)','Myanmar','Australia'], price:{amount:300,unit:'$/kg (oxide)',year:2024}, primaryUses:['NdFeB magnet additive (98%)','Lasers','Nuclear control rods','Lighting'] },
   Ho: { production:{note:'Rare earth — ~50 tonnes/yr'}, topProducers:['China','Myanmar'], primaryUses:['Magnets','Medical lasers','Nuclear reactors','Spectrophotometry'] },
   Er: { production:{note:'Rare earth — ~800 tonnes/yr'}, topProducers:['China','Myanmar','Australia'], primaryUses:['Fiber optic amplifiers','Lasers','Nuclear','Glass colorant (pink)'] },
   Tm: { production:{note:'Rare earth — ~15 tonnes/yr (rarest commercial)'}, topProducers:['China'], primaryUses:['Portable X-ray machines','Lasers','Research'] },
   Yb: { production:{note:'Rare earth — ~50 tonnes/yr'}, topProducers:['China','Myanmar'], primaryUses:['Fiber optic amplifiers','Atomic clocks','Lasers','Stress gauges'] },
   Lu: { production:{note:'Rare earth — ~10 tonnes/yr'}, topProducers:['China'], price:{amount:900,unit:'$/kg (oxide)',year:2024}, primaryUses:['PET scan detectors','Oil refining catalysts','LED phosphors','Research'] },
   Hf: { production:{annual:'70 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Included with zirconium',yearsSupply:null}, topProducers:['France','USA','Ukraine','Russia'], price:{amount:1200,unit:'$/kg',year:2024}, primaryUses:['Nuclear reactor control rods (47%)','Superalloys','Plasma cutting','Microprocessors'] },
   Ta: { production:{annual:'1,800 tonnes',unit:'tonnes/yr'}, reserves:{amount:'140,000 tonnes',yearsSupply:78}, topProducers:['DRC (37%)','Brazil','Rwanda','Nigeria','China'], price:{amount:190,unit:'$/kg',year:2024}, primaryUses:['Capacitors (60%)','Superalloys','Surgical implants','Chemical equipment'] },
   W: { production:{annual:'84,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'3.7 million tonnes',yearsSupply:44}, topProducers:['China (82%)','Vietnam','Russia','Bolivia','Austria'], price:{amount:35,unit:'$/kg',year:2024}, primaryUses:['Cemented carbide tools (60%)','Steel alloys','Mill products','Chemicals'] },
   Re: { production:{annual:'56 tonnes',unit:'tonnes/yr'}, reserves:{amount:'2,500 tonnes',yearsSupply:45}, topProducers:['Chile','USA','Poland','Uzbekistan','Kazakhstan'], price:{amount:1600,unit:'$/kg',year:2024}, primaryUses:['Jet engine superalloys (80%)','Catalysts','Thermocouples','Filaments'] },
   Os: { production:{note:'PGM byproduct — ~1 tonne/yr'}, topProducers:['South Africa','Russia'], price:{amount:12000,unit:'$/kg',year:2024}, primaryUses:['Fountain pen tips','Electrical contacts','Catalysts','Fingerprint detection'] },
   Ir: { production:{annual:'7.5 tonnes',unit:'tonnes/yr'}, reserves:{amount:'Included with PGMs',yearsSupply:null}, topProducers:['South Africa (85%)','Russia','Zimbabwe'], price:{amount:160000,unit:'$/kg',year:2024}, primaryUses:['Spark plugs','Crucibles','Electrochemistry','Fountain pen nibs'] },
   Pt: { production:{annual:'190 tonnes',unit:'tonnes/yr'}, reserves:{amount:'13,000 tonnes',yearsSupply:68}, topProducers:['South Africa (72%)','Russia','Zimbabwe','Canada','USA'], price:{amount:30000,unit:'$/kg',year:2024}, primaryUses:['Catalytic converters (40%)','Jewelry (30%)','Industrial catalysts','Fuel cells'] },
   Au: { production:{annual:'3,300 tonnes',unit:'tonnes/yr'}, reserves:{amount:'59,000 tonnes',yearsSupply:18}, topProducers:['China','Australia','Russia','Canada','USA'], price:{amount:65000,unit:'$/kg',year:2024}, primaryUses:['Jewelry (50%)','Investment/central banks (25%)','Electronics (10%)','Dentistry'] },
   Hg: { production:{annual:'3,700 tonnes',unit:'tonnes/yr'}, reserves:{amount:'580,000 tonnes',yearsSupply:157}, topProducers:['China (80%)','Mexico','Tajikistan'], price:{amount:30,unit:'$/kg',year:2024}, primaryUses:['Small-scale gold mining (37%)','VCM production','Fluorescent lamps','Dental amalgam'] },
   Tl: { production:{note:'Byproduct of zinc/lead smelting — ~10 tonnes/yr'}, topProducers:['China','Japan','Belgium'], primaryUses:['Semiconductors','Infrared optics','Superconductor research','Medical imaging'] },
   Pb: { production:{annual:'4.6 million tonnes',unit:'tonnes/yr'}, reserves:{amount:'85 million tonnes',yearsSupply:18}, topProducers:['China (43%)','Australia','USA','Peru','Mexico'], price:{amount:2100,unit:'$/tonne',year:2024}, primaryUses:['Lead-acid batteries (85%)','Ammunition','Cable sheathing','Radiation shielding'] },
   Bi: { production:{annual:'20,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'370,000 tonnes',yearsSupply:19}, topProducers:['China (80%)','Vietnam','Mexico','Japan','Bolivia'], price:{amount:6,unit:'$/kg',year:2024}, primaryUses:['Pharmaceuticals (Pepto-Bismol)','Cosmetics','Lead-free solder','Fire sprinkler alloys'] },
   Po: { production:{note:'Produced in nuclear reactors — ~100 g/yr globally'}, primaryUses:['Static eliminators','Nuclear research','Satellite heat sources'] },
   At: { production:{note:'Synthetic — total worldwide less than 1 μg at any time'}, primaryUses:['Cancer radiotherapy research'] },
   Rn: { production:{note:'Naturally occurring radioactive decay product — not commercially produced'}, primaryUses:['Radon testing/mitigation','Earthquake prediction research'] },
   Fr: { production:{note:'Synthetic — most unstable natural element, produced in accelerators'}, primaryUses:['Research only'] },
   Ra: { production:{note:'Not commercially produced — extremely radioactive'}, primaryUses:['Cancer treatment (historic)','Research'] },
   Ac: { production:{note:'Trace amounts from nuclear reactors for research'}, primaryUses:['Targeted alpha therapy research','Neutron source'] },
   Th: { production:{annual:'~10,000 tonnes (byproduct)',unit:'tonnes/yr'}, reserves:{amount:'6.4 million tonnes',yearsSupply:640}, topProducers:['India','Brazil','Australia','USA','Egypt'], price:{amount:80,unit:'$/kg',year:2024}, primaryUses:['Potential nuclear fuel','Gas mantles','Welding electrodes','Optical coatings'] },
   Pa: { production:{note:'Extremely rare — ~130 g total isolated historically'}, primaryUses:['Research','Ocean sediment dating'] },
   U: { production:{annual:'58,000 tonnes',unit:'tonnes/yr'}, reserves:{amount:'6.1 million tonnes',yearsSupply:105}, topProducers:['Kazakhstan (43%)','Namibia','Canada','Australia','Uzbekistan'], price:{amount:130,unit:'$/kg (U₃O₈)',year:2024}, primaryUses:['Nuclear power (96%)','Research reactors','Nuclear weapons','Radiation shielding (depleted)'] },
   Np: { production:{note:'Byproduct of nuclear reactors — a few kg/yr'}, primaryUses:['Neutron detection','Precursor to Pu-238','Research'] },
   Pu: { production:{note:'Produced in nuclear reactors — ~70 tonnes/yr worldwide'}, primaryUses:['Nuclear weapons','MOX fuel','RTGs for space probes','Research'] },
   Am: { production:{note:'Produced in nuclear reactors — ~10 g/yr for commercial use'}, primaryUses:['Smoke detectors','Neutron sources','Research'] },
   Cm: { production:{note:'Produced in nuclear reactors — milligram quantities'}, primaryUses:['Space probe RTGs','Alpha particle source','Research'] },
   Bk: { production:{note:'Synthetic — microgram quantities in nuclear reactors'}, primaryUses:['Research','Target for heavier element synthesis'] },
   Cf: { production:{note:'Produced at ORNL and RIAR — ~0.5 g/yr globally'}, price:{amount:27000000,unit:'$/g',year:2024}, primaryUses:['Neutron source (oil well logging)','Nuclear reactor startup','Cancer treatment','Mineral analysis'] },
   Es: { production:{note:'Synthetic — microgram quantities'}, primaryUses:['Research only'] },
   Fm: { production:{note:'Synthetic — produced in nuclear reactors'}, primaryUses:['Research only'] },
   Md: { production:{note:'Synthetic — only atoms at a time in particle accelerators'}, primaryUses:['Research only'] },
   No: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Lr: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Rf: { production:{note:'Synthetic — atoms at a time in particle accelerators'}, primaryUses:['Research only'] },
   Db: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Sg: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Bh: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Hs: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Mt: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Ds: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Rg: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Cn: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Nh: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Fl: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Mc: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Lv: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Ts: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
   Og: { production:{note:'Synthetic — atoms at a time'}, primaryUses:['Research only'] },
  };

  // Merge world data into ELEMENTS
  ELEMENTS.forEach(e => {
   const wd = ELEMENT_WORLD_DATA[e.symbol];
   if (wd) Object.assign(e, { worldData: wd });
  });

  // ══════════════════════════════════════
  // MATERIAL WORLD DATA (USGS/World Bank 2024)
  // ══════════════════════════════════════
  const MATERIAL_WORLD_DATA = {
   'Iron Ore': { production:{annual:'2.5 billion tonnes',growth:'+1.5%/yr'}, energyCost:'0.5 GJ per tonne (mining)', topProducers:['Australia (37%)','Brazil (17%)','China (14%)','India (10%)'], co2:'0.04 tonnes CO₂ per tonne' },
   'Steel': { production:{annual:'1.9 billion tonnes',growth:'+3.2%/yr'}, inputChain:[{material:'Iron Ore',ratio:'2.5 tonnes per tonne steel'},{material:'Coal/Coke',ratio:'0.6 tonnes per tonne steel'},{material:'Limestone',ratio:'0.2 tonnes per tonne steel'}], energyCost:'20 GJ per tonne', topProducers:['China (54%)','India (7%)','Japan (5%)','USA (5%)'], co2:'1.85 tonnes CO₂ per tonne steel' },
   'Wood / Timber': { production:{annual:'4 billion m³ (roundwood)',growth:'+0.5%/yr'}, topProducers:['USA','India','China','Brazil','Russia'], co2:'Net carbon sink when sustainably managed' },
   'Lumber': { production:{annual:'500 million m³ (sawnwood)',growth:'+1.0%/yr'}, energyCost:'1.5 GJ per m³', topProducers:['USA','China','Canada','Russia','Germany'], co2:'Stores ~250 kg CO₂ per m³' },
   'Sand': { production:{annual:'50 billion tonnes',growth:'+5.5%/yr'}, topProducers:['China','USA','India','EU','Australia'], co2:'Minimal (extraction only)' },
   'Glass': { production:{annual:'200 million tonnes',growth:'+3.5%/yr'}, inputChain:[{material:'Sand (SiO₂)',ratio:'72%'},{material:'Soda ash',ratio:'14%'},{material:'Limestone',ratio:'10%'}], energyCost:'7-10 GJ per tonne', topProducers:['China (50%)','EU','USA','India','Japan'], co2:'0.6 tonnes CO₂ per tonne' },
   'Clay': { production:{annual:'Billions of tonnes (not fully tracked)',growth:'Stable'}, topProducers:['China','USA','Germany','UK','India'] },
   'Concrete': { production:{annual:'14 billion m³ (~33 billion tonnes)',growth:'+2.5%/yr'}, inputChain:[{material:'Cement',ratio:'10-15%'},{material:'Sand',ratio:'25-30%'},{material:'Gravel',ratio:'35-40%'},{material:'Water',ratio:'15-20%'}], energyCost:'1.5-2 GJ per tonne', topProducers:['China (55%)','India','USA','Turkey','Brazil'], co2:'0.1-0.2 tonnes CO₂ per tonne concrete' },
   'Aluminum Ore (Bauxite)': { production:{annual:'380 million tonnes',growth:'+2.5%/yr'}, topProducers:['Australia (28%)','Guinea (22%)','China (16%)','Brazil','Indonesia'], co2:'0.01 tonnes CO₂ per tonne (mining)' },
   'Aluminum': { production:{annual:'69 million tonnes',growth:'+2.8%/yr'}, inputChain:[{material:'Bauxite',ratio:'4-5 tonnes per tonne aluminum'},{material:'Alumina',ratio:'2 tonnes per tonne aluminum'},{material:'Electricity',ratio:'14 MWh per tonne'}], energyCost:'170 GJ per tonne', topProducers:['China (57%)','India (6%)','Russia (5%)','Canada (5%)','UAE (4%)'], co2:'12 tonnes CO₂ per tonne (global avg)' },
   'Copper Ore (Chalcopyrite)': { production:{annual:'22 million tonnes (Cu content)',growth:'+2.0%/yr'}, topProducers:['Chile (24%)','DRC (12%)','Peru (10%)','China','Indonesia'], co2:'Varies by mine' },
   'Brass': { production:{annual:'~6 million tonnes',growth:'+2%/yr'}, inputChain:[{material:'Copper',ratio:'60-70%'},{material:'Zinc',ratio:'30-40%'}], energyCost:'8 GJ per tonne', topProducers:['China','Germany','Japan','USA','Italy'] },
   'Bronze': { production:{annual:'~1 million tonnes',growth:'+1%/yr'}, inputChain:[{material:'Copper',ratio:'88-95%'},{material:'Tin',ratio:'5-12%'}], energyCost:'9 GJ per tonne', topProducers:['China','USA','Japan','Germany','South Korea'] },
   'Stainless Steel': { production:{annual:'58 million tonnes',growth:'+4.5%/yr'}, inputChain:[{material:'Iron/Steel',ratio:'70-75%'},{material:'Chromium',ratio:'10.5-18%'},{material:'Nickel',ratio:'8-12%'}], energyCost:'30 GJ per tonne', topProducers:['China (56%)','India (7%)','Japan (5%)','EU','USA'], co2:'2.8 tonnes CO₂ per tonne' },
   'Titanium': { production:{annual:'220,000 tonnes (metal)',growth:'+3%/yr'}, inputChain:[{material:'Ilmenite/Rutile ore',ratio:'~10 tonnes ore per tonne metal'},{material:'Magnesium (Kroll process)',ratio:'1 tonne per tonne Ti'}], energyCost:'360 GJ per tonne', topProducers:['China','Russia','Japan','Kazakhstan','USA'], co2:'8.6 tonnes CO₂ per tonne' },
   'Zinc': { production:{annual:'13 million tonnes',growth:'+1.5%/yr'}, inputChain:[{material:'Sphalerite ore',ratio:'~3 tonnes per tonne zinc'}], energyCost:'35 GJ per tonne', topProducers:['China (42%)','Peru','Australia','India','USA'], co2:'2.5 tonnes CO₂ per tonne' },
   'Lead': { production:{annual:'4.6 million tonnes',growth:'+1.0%/yr'}, inputChain:[{material:'Galena ore',ratio:'~3 tonnes per tonne lead'}], energyCost:'15 GJ per tonne', topProducers:['China (43%)','Australia','USA','Peru','Mexico'], co2:'1.0 tonnes CO₂ per tonne' },
   'Tin': { production:{annual:'310,000 tonnes',growth:'+1.2%/yr'}, inputChain:[{material:'Cassiterite ore',ratio:'~5 tonnes per tonne tin'}], energyCost:'20 GJ per tonne', topProducers:['China (30%)','Indonesia','Myanmar','Peru','DRC'], co2:'1.5 tonnes CO₂ per tonne' },
   'Nickel': { production:{annual:'3.6 million tonnes',growth:'+6%/yr'}, inputChain:[{material:'Laterite/Sulfide ore',ratio:'~50-100 tonnes ore per tonne Ni'}], energyCost:'110 GJ per tonne', topProducers:['Indonesia (49%)','Philippines','Russia','New Caledonia','Australia'], co2:'10 tonnes CO₂ per tonne' },
   'Cotton': { production:{annual:'25 million tonnes',growth:'+0.5%/yr'}, energyCost:'55 GJ per tonne (field to fabric)', topProducers:['India (23%)','China (22%)','USA (14%)','Brazil','Pakistan'], co2:'5 tonnes CO₂ per tonne' },
   'Wool': { production:{annual:'1.1 million tonnes (greasy)',growth:'-0.5%/yr'}, topProducers:['Australia (21%)','China (18%)','New Zealand','UK','Turkey'], co2:'26 kg CO₂ per kg (high due to methane)' },
   'Leather': { production:{annual:'20 billion sq ft',growth:'+1%/yr'}, topProducers:['China','Italy','Brazil','India','Vietnam'] },
   'Rubber (Natural)': { production:{annual:'14 million tonnes',growth:'+2%/yr'}, topProducers:['Thailand (34%)','Indonesia (24%)','Vietnam','China','India'], co2:'Low (trees absorb CO₂)' },
   'Paper': { production:{annual:'410 million tonnes',growth:'+0.5%/yr'}, inputChain:[{material:'Wood',ratio:'2-3 tonnes per tonne paper'},{material:'Water',ratio:'10-20 m³ per tonne'}], energyCost:'20-40 GJ per tonne', topProducers:['China (28%)','USA (17%)','Japan','Germany','India'], co2:'0.5-1.5 tonnes CO₂ per tonne' },
   'Bamboo': { production:{annual:'~40 million tonnes',growth:'+3%/yr'}, topProducers:['China (65%)','India','Myanmar','Thailand','Vietnam'] },
   'Hemp': { production:{annual:'280,000 tonnes (fiber)',growth:'+15%/yr'}, topProducers:['China','Canada','France','EU','USA'] },
   'Cork': { production:{annual:'200,000 tonnes',growth:'+1%/yr'}, topProducers:['Portugal (49%)','Spain (26%)','Morocco','Algeria','Tunisia'] },
   'Granite': { production:{annual:'~160 million tonnes',growth:'+3%/yr'}, topProducers:['China','India','Brazil','Italy','Spain'] },
   'Marble': { production:{annual:'~100 million tonnes',growth:'+2%/yr'}, topProducers:['China','India','Turkey','Italy','Iran'] },
   'Limestone': { production:{annual:'~7 billion tonnes',growth:'+2%/yr'}, topProducers:['China','USA','India','Russia','Japan'] },
   'Cement': { production:{annual:'4.1 billion tonnes',growth:'+1.5%/yr'}, inputChain:[{material:'Limestone',ratio:'1.5 tonnes per tonne cement'},{material:'Clay',ratio:'0.3 tonnes per tonne'},{material:'Gypsum',ratio:'5%'}], energyCost:'4.5 GJ per tonne', topProducers:['China (55%)','India (8%)','Vietnam','USA','Turkey'], co2:'0.6 tonnes CO₂ per tonne cement' },
   'Brick': { production:{annual:'1.5 trillion bricks/yr',growth:'+2%/yr'}, energyCost:'2-5 GJ per 1000 bricks', topProducers:['China (60%)','India','Pakistan','Bangladesh','Indonesia'], co2:'0.2 kg CO₂ per brick' },
   'Porcelain': { production:{annual:'~400 million tonnes (all ceramics)',growth:'+3%/yr'}, topProducers:['China (65%)','Italy','Spain','India','Turkey'] },
   'Fiberglass': { production:{annual:'~6 million tonnes',growth:'+5%/yr'}, inputChain:[{material:'Glass fiber',ratio:'60-70%'},{material:'Polymer resin',ratio:'30-40%'}], energyCost:'30 GJ per tonne', topProducers:['China','USA','EU','India','Japan'] },
   'Plastic (Polyethylene)': { production:{annual:'100 million tonnes (PE only); 400M total plastics',growth:'+3.5%/yr'}, inputChain:[{material:'Crude oil/natural gas',ratio:'~2 tonnes oil per tonne PE'}], energyCost:'70-80 GJ per tonne', topProducers:['China (32%)','USA','EU','South Korea','Saudi Arabia'], co2:'2-3 tonnes CO₂ per tonne' },
   'PVC (Polyvinyl Chloride)': { production:{annual:'45 million tonnes',growth:'+3%/yr'}, inputChain:[{material:'Ethylene (from oil)',ratio:'43%'},{material:'Chlorine (from salt)',ratio:'57%'}], energyCost:'55 GJ per tonne', topProducers:['China (40%)','USA','EU','India','Japan'], co2:'2 tonnes CO₂ per tonne' },
   'Nylon': { production:{annual:'8 million tonnes',growth:'+4%/yr'}, energyCost:'120 GJ per tonne', topProducers:['China','USA','EU','Taiwan','South Korea'], co2:'7 tonnes CO₂ per tonne' },
   'Carbon Fiber': { production:{annual:'120,000 tonnes',growth:'+12%/yr'}, inputChain:[{material:'PAN precursor',ratio:'2 tonnes per tonne CF'},{material:'Energy (carbonization)',ratio:'Very high'}], energyCost:'600 GJ per tonne', topProducers:['Japan (Toray 34%)','China','USA','EU','Taiwan'], co2:'20-40 tonnes CO₂ per tonne' },
   'Kevlar': { production:{annual:'~70,000 tonnes (all aramids)',growth:'+5%/yr'}, energyCost:'200 GJ per tonne', topProducers:['USA (DuPont)','Japan (Teijin)','South Korea','EU'] },
   'Plywood': { production:{annual:'170 million m³',growth:'+3%/yr'}, inputChain:[{material:'Wood veneers',ratio:'~1.5 m³ log per m³ plywood'},{material:'Adhesive',ratio:'5-10% by weight'}], energyCost:'5 GJ per m³', topProducers:['China (66%)','Indonesia','Russia','USA','India'] },
   'MDF (Medium-Density Fiberboard)': { production:{annual:'100 million m³',growth:'+4%/yr'}, inputChain:[{material:'Wood fiber',ratio:'85-90%'},{material:'UF resin',ratio:'10-15%'}], energyCost:'6 GJ per m³', topProducers:['China (50%)','Turkey','Brazil','EU','USA'] },
   'Coal': { production:{annual:'8.7 billion tonnes',growth:'-0.5%/yr'}, topProducers:['China (52%)','India (10%)','Indonesia (8%)','USA (6%)','Australia (6%)'], co2:'2.86 tonnes CO₂ per tonne burned' },
   'Petroleum (Crude Oil)': { production:{annual:'4.4 billion tonnes (100M barrels/day)',growth:'+0.5%/yr'}, topProducers:['USA (20%)','Saudi Arabia (12%)','Russia (11%)','Canada','Iraq'], co2:'3.1 tonnes CO₂ per tonne burned' },
   'Natural Gas': { production:{annual:'4.0 trillion m³',growth:'+2%/yr'}, topProducers:['USA (25%)','Russia (15%)','Iran','China','Canada'], co2:'2.0 tonnes CO₂ per 1000 m³ burned' },
  };

    const ELEMENT_CATEGORIES = [...new Set(ELEMENTS.map(e => e.category))];
  let elementCatFilter = '';
  let selectedElement = null;

  function renderElementPills() {
   const pills = document.getElementById('element-cat-pills');
   pills.innerHTML = `<span class="filter-pill ${elementCatFilter===''?'active':''}" onclick="elementCatFilter='';renderElements()">All</span>` +
    ELEMENT_CATEGORIES.map(c => `<span class="filter-pill ${elementCatFilter===c?'active':''}" onclick="elementCatFilter='${c}';renderElements()">${c}</span>`).join('');
  }

  function renderElements() {
   const filter = (document.getElementById('element-filter').value || '').toLowerCase();
   const filtered = ELEMENTS.filter(e => {
    if (elementCatFilter && e.category !== elementCatFilter) return false;
    if (filter && !e.name.toLowerCase().includes(filter) && !e.symbol.toLowerCase().includes(filter) && !String(e.number).includes(filter)) return false;
    return true;
   });
   renderElementPills();
   document.getElementById('element-grid').innerHTML = filtered.map(e =>
    `<div class="element-card el-cat-${e.category}" onclick="showElementDetail('${e.symbol}')" title="${e.name}">
     <span class="el-number">${e.number}</span>
     <div class="el-symbol">${e.symbol}</div>
     <div class="el-name">${e.name}</div>
     <div class="el-mass">${e.mass}</div>
    </div>`
   ).join('');
  }

  function showElementDetail(symbol) {
   const e = ELEMENTS.find(el => el.symbol === symbol);
   if (!e) return;
   if (selectedElement === symbol) { selectedElement = null; document.getElementById('element-detail-slot').innerHTML = ''; return; }
   selectedElement = symbol;
   const srcLinks = e.sources.map(s => `<a href="${s.url}" target="_blank" rel="noopener" style="color:var(--accent);font-size:0.75rem;margin-right:0.6rem;">${s.name} ↗</a>`).join('');
   document.getElementById('element-detail-slot').innerHTML = `
    <div class="element-detail">
     <div style="display:flex;justify-content:space-between;align-items:start;">
      <h3><span class="el-cat-${e.category}" style="border:none;display:inline;"><span class="el-symbol" style="font-size:1.4rem;">${e.symbol}</span></span> ${e.name} <span style="color:var(--text-muted);font-weight:400;font-size:0.85rem;">#${e.number}</span></h3>
      <button onclick="selectedElement=null;document.getElementById('element-detail-slot').innerHTML=''" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:1.1rem;">Close</button>
     </div>
     <p style="font-size:0.82rem;color:var(--text-muted);margin:0.3rem 0 0.6rem;">${e.description}</p>
     <div class="el-props">
      <div class="el-prop"><dt>Mass</dt><dd>${e.mass} u</dd></div>
      <div class="el-prop"><dt>Phase (STP)</dt><dd>${e.phase}</dd></div>
      <div class="el-prop"><dt>Density</dt><dd>${e.density}</dd></div>
      <div class="el-prop"><dt>Melting Pt</dt><dd>${e.meltingPoint}</dd></div>
      <div class="el-prop"><dt>Boiling Pt</dt><dd>${e.boilingPoint}</dd></div>
      <div class="el-prop"><dt>Category</dt><dd>${e.category}</dd></div>
      <div class="el-prop"><dt>Discovered</dt><dd>${e.discovered}</dd></div>
      <div class="el-prop"><dt>Discoverer</dt><dd>${e.discoverer}</dd></div>
     </div>
     <div style="margin:0.6rem 0;"><strong style="font-size:0.75rem;color:var(--text-muted);">USES:</strong>
      <div style="display:flex;flex-wrap:wrap;gap:0.3rem;margin-top:0.3rem;">
       ${e.uses.map(u => `<span style="background:rgba(255,136,17,0.1);color:var(--accent);font-size:0.7rem;padding:0.15rem 0.5rem;border-radius:10px;">${u}</span>`).join('')}
      </div>
     </div>
     ${e.worldData ? `
     <details style="margin-top:0.6rem;border:1px solid var(--border);border-radius:8px;padding:0.4rem 0.6rem;background:rgba(0,255,136,0.03);">
      <summary style="cursor:pointer;font-size:0.8rem;font-weight:600;color:var(--accent);">🌍 World Data</summary>
      <div style="margin-top:0.4rem;font-size:0.75rem;line-height:1.6;">
       ${e.worldData.production ? (e.worldData.production.annual
        ? '<div><strong>Annual Production:</strong> '+e.worldData.production.annual+'</div>'
        : '<div><em>'+e.worldData.production.note+'</em></div>') : ''}
       ${e.worldData.reserves ? (e.worldData.reserves.amount
        ? '<div><strong>Reserves:</strong> '+e.worldData.reserves.amount
         +(e.worldData.reserves.yearsSupply!=null
          ? ' <span style="padding:0.1rem 0.4rem;border-radius:8px;font-size:0.7rem;font-weight:600;background:'
           +(e.worldData.reserves.yearsSupply>100?'rgba(0,200,83,0.2);color:#0c6':'rgba(255,160,0,0.2);color:#f90')
           +'">~'+e.worldData.reserves.yearsSupply+' years</span>'
          : '')+'</div>' : '') : ''}
       ${e.worldData.topProducers ? '<div><strong>Top Producers:</strong> '+e.worldData.topProducers.join(', ')+'</div>' : ''}
       ${e.worldData.price && e.worldData.price.amount ? '<div><strong>Price:</strong> '+e.worldData.price.amount.toLocaleString()+' '+e.worldData.price.unit+' ('+e.worldData.price.year+')</div>' : ''}
       ${e.worldData.primaryUses ? '<div><strong>Primary Uses:</strong> '+e.worldData.primaryUses.join(', ')+'</div>' : ''}
      </div>
     </details>` : ''}
     <div style="margin-top:0.5rem;">${srcLinks}</div>
    </div>`;
  }
  renderElements();

  // ══════════════════════════════════════
  // MATERIALS CATALOG
  // ══════════════════════════════════════
  const MATERIALS = [
   { name:'Iron Ore', type:'raw', category:'Metal',
    components:[], sources_natural:['Open-pit mines','Underground deposits','BIF formations'],
    properties:{ iron_content:'25-70%', density:'4.0-5.5 g/cm³', hardness:'5-6.5 Mohs' },
    uses:['Steel production','Pigments','Cement additive'],
    processing:'Mining → Crushing → Beneficiation → Iron ore concentrate',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/iron-ore-statistics-and-information'}] },
   { name:'Steel', type:'processed', category:'Metal',
    components:['Iron','Carbon'], sources_natural:['Iron ore deposits','Coal mines'],
    properties:{ tensile_strength:'400-550 MPa', density:'7.85 g/cm³', melting_point:'1370-1510 °C' },
    uses:['Construction','Vehicles','Tools','Bridges'],
    processing:'Iron ore → Blast furnace → Pig iron → Basic oxygen steelmaking → Steel',
    references:[{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=1b3c3a1bc9d24cccb03b1bacbbcdfa8c'}] },
   { name:'Wood / Timber', type:'raw', category:'Organic',
    components:[], sources_natural:['Forests','Tree plantations'],
    properties:{ density:'0.4-0.9 g/cm³', tensile_strength:'40-140 MPa', moisture:'12-20%' },
    uses:['Construction','Furniture','Paper','Fuel'],
    processing:'Felling → Debarking → Sawing → Seasoning',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Wood'}] },
   { name:'Lumber', type:'processed', category:'Organic',
    components:['Wood / Timber'], sources_natural:['Managed forests','Sawmills'],
    properties:{ density:'0.35-0.6 g/cm³', moisture:'6-12%', standard_sizes:'2×4, 2×6, 4×4 etc.' },
    uses:['Framing','Decking','Fencing','Shelving'],
    processing:'Timber → Sawmill → Kiln drying → Planing → Grading → Lumber',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Lumber'}] },
   { name:'Sand', type:'raw', category:'Mineral',
    components:[], sources_natural:['Beaches','Riverbeds','Quarries','Deserts'],
    properties:{ composition:'Mostly SiO₂', grain_size:'0.1-2 mm', density:'1.5-1.7 g/cm³' },
    uses:['Glass making','Concrete','Foundry casting','Filtration'],
    processing:'Extraction → Washing → Screening → Grading',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/sand-and-gravel-statistics-and-information'}] },
   { name:'Glass', type:'processed', category:'Mineral',
    components:['Sand','Soda ash','Limestone'], sources_natural:['Sand quarries'],
    properties:{ density:'2.5 g/cm³', melting_point:'~1700 °C', transparency:'High (visible spectrum)' },
    uses:['Windows','Bottles','Optics','Screens'],
    processing:'Sand + Soda ash + Limestone → Furnace (1700°C) → Molten glass → Forming → Annealing → Glass',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Glass'}] },
   { name:'Clay', type:'raw', category:'Mineral',
    components:[], sources_natural:['Riverbanks','Weathered rock deposits','Sedimentary layers'],
    properties:{ particle_size:'<0.002 mm', plasticity:'High when wet', density:'1.8-2.6 g/cm³' },
    uses:['Pottery','Bricks','Ceramics','Sculpture'],
    processing:'Extraction → Weathering → Pugging → Shaping',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Clay'}] },
   { name:'Concrete', type:'composite', category:'Mineral',
    components:['Sand','Gravel','Cement','Water'], sources_natural:['Quarries','Cement plants'],
    properties:{ compressive_strength:'20-40 MPa', density:'2.3-2.5 g/cm³', curing_time:'28 days full strength' },
    uses:['Foundations','Roads','Dams','Buildings'],
    processing:'Cement + Sand + Gravel + Water → Mixing → Pouring → Curing → Concrete',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Concrete'}] },
{ name:'Aluminum Ore (Bauxite)', type:'raw', category:'Metal',
    components:[], sources_natural:['Open-pit mines in Australia, Guinea, Brazil, Jamaica'],
    properties:{ aluminum_content:'30-54% Al₂O₃', density:'2.0-2.5 g/cm³', hardness:'1-3 Mohs' },
    uses:['Aluminum production','Abrasives','Cement','Refractories'],
    processing:'Mining → Crushing → Washing → Bayer process → Alumina',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/bauxite-and-alumina-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Bauxite'}] },
   { name:'Aluminum', type:'processed', category:'Metal',
    components:['Bauxite'], sources_natural:['Bauxite mines in Australia, Guinea, Brazil'],
    properties:{ density:'2.7 g/cm³', tensile_strength:'70-700 MPa', melting_point:'660 °C', conductivity:'High' },
    uses:['Aircraft','Cans','Foil','Window frames','Electronics'],
    processing:'Bauxite → Bayer process → Alumina → Hall-Héroult electrolysis → Aluminum',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/aluminum-statistics-and-information'},{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=0cd1edf33ac145ee93a0aa06bcdbc073'}] },
   { name:'Copper Ore (Chalcopyrite)', type:'raw', category:'Metal',
    components:[], sources_natural:['Porphyry deposits in Chile, Peru, USA, DRC'],
    properties:{ copper_content:'25-35% Cu', density:'4.1-4.3 g/cm³', hardness:'3.5-4 Mohs' },
    uses:['Copper production','Sulfuric acid byproduct'],
    processing:'Mining → Crushing → Flotation → Concentrate → Smelting',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/copper-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Chalcopyrite'}] },
   { name:'Brass', type:'processed', category:'Metal',
    components:['Copper','Zinc'], sources_natural:['Copper and zinc ores'],
    properties:{ density:'8.4-8.7 g/cm³', tensile_strength:'338-469 MPa', melting_point:'900-940 °C', conductivity:'Moderate' },
    uses:['Plumbing fittings','Musical instruments','Ammunition casings','Decorative hardware'],
    processing:'Copper + Zinc → Melting → Alloying → Casting/Rolling → Brass',
    references:[{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=d3bd4617903543ada92f4c101c2a20e5'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Brass'}] },
   { name:'Bronze', type:'processed', category:'Metal',
    components:['Copper','Tin'], sources_natural:['Copper and tin ores'],
    properties:{ density:'7.4-8.9 g/cm³', tensile_strength:'303-517 MPa', melting_point:'950-1050 °C', conductivity:'Moderate' },
    uses:['Bearings','Sculptures','Ship propellers','Springs'],
    processing:'Copper + Tin → Melting → Alloying → Casting → Bronze',
    references:[{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=b607236c21e545ca9ebf7875cbe0300d'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Bronze'}] },
   { name:'Stainless Steel', type:'processed', category:'Metal',
    components:['Iron','Chromium','Nickel','Carbon'], sources_natural:['Iron, chromium, and nickel ores'],
    properties:{ density:'7.75-8.1 g/cm³', tensile_strength:'515-827 MPa', melting_point:'1400-1530 °C', conductivity:'Low-Moderate' },
    uses:['Kitchen appliances','Medical instruments','Architecture','Chemical tanks'],
    processing:'Iron + Chromium (10.5%+) + Nickel → Electric arc furnace → Rolling → Stainless Steel',
    references:[{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=abc4415b0f8b490387e3c922237098da'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Stainless_steel'}] },
   { name:'Titanium', type:'processed', category:'Metal',
    components:['Ilmenite','Rutile'], sources_natural:['Mineral sands in Australia, South Africa, Canada'],
    properties:{ density:'4.506 g/cm³', tensile_strength:'434-1103 MPa', melting_point:'1668 °C', conductivity:'Low' },
    uses:['Aerospace','Medical implants','Jewelry','Chemical processing'],
    processing:'Rutile/Ilmenite → Chlorination → Kroll process (Mg reduction) → Titanium sponge → Melting',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/titanium-statistics-and-information'},{name:'MatWeb',url:'https://matweb.com/search/DataSheet.aspx?MatGUID=66a15d609a3f4c829cb6ad08f0dafc01'}] },
   { name:'Zinc', type:'processed', category:'Metal',
    components:['Sphalerite'], sources_natural:['Zinc mines in China, Australia, Peru'],
    properties:{ density:'7.134 g/cm³', tensile_strength:'37 MPa', melting_point:'419.5 °C', conductivity:'Moderate' },
    uses:['Galvanizing steel','Die casting','Brass alloy','Batteries'],
    processing:'Sphalerite → Roasting → Leaching → Electrolysis → Zinc',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/zinc-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Zinc'}] },
   { name:'Lead', type:'processed', category:'Metal',
    components:['Galena'], sources_natural:['Lead-zinc mines in China, Australia, USA'],
    properties:{ density:'11.34 g/cm³', tensile_strength:'17 MPa', melting_point:'327.5 °C', conductivity:'Low' },
    uses:['Lead-acid batteries','Radiation shielding','Ammunition','Cable sheathing'],
    processing:'Galena → Roasting → Smelting → Refining → Lead',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/lead-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Lead'}] },
   { name:'Tin', type:'processed', category:'Metal',
    components:['Cassiterite'], sources_natural:['Alluvial and hard-rock mines in China, Indonesia, Myanmar'],
    properties:{ density:'7.287 g/cm³', tensile_strength:'11-15 MPa', melting_point:'231.9 °C', conductivity:'Moderate' },
    uses:['Tin plating (tin cans)','Solder','Bronze alloy','Pewter'],
    processing:'Cassiterite → Smelting with carbon → Refining → Tin',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/tin-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Tin'}] },
   { name:'Nickel', type:'processed', category:'Metal',
    components:['Pentlandite','Laterite ores'], sources_natural:['Mines in Indonesia, Philippines, Russia, Canada'],
    properties:{ density:'8.908 g/cm³', tensile_strength:'317-462 MPa', melting_point:'1455 °C', conductivity:'Moderate' },
    uses:['Stainless steel','Batteries','Coins','Superalloys'],
    processing:'Laterite/Sulfide ore → Roasting/Leaching → Smelting → Electrolytic refining → Nickel',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/nickel-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Nickel'}] },
   { name:'Cotton', type:'raw', category:'Organic',
    components:[], sources_natural:['Cotton farms in India, China, USA, Brazil'],
    properties:{ density:'1.5-1.6 g/cm³', tensile_strength:'287-597 MPa', moisture_regain:'8.5%', fiber_length:'10-65 mm' },
    uses:['Clothing','Towels','Medical supplies','Industrial fabrics'],
    processing:'Harvesting → Ginning → Carding → Spinning → Weaving/Knitting',
    references:[{name:'USDA',url:'https://usda.gov/topics/farming/crop-production'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Cotton'}] },
   { name:'Wool', type:'raw', category:'Organic',
    components:[], sources_natural:['Sheep farms in Australia, China, New Zealand'],
    properties:{ density:'1.3 g/cm³', fiber_diameter:'15-40 μm', moisture_regain:'13-16%', flame_resistance:'Self-extinguishing' },
    uses:['Clothing','Blankets','Carpets','Insulation'],
    processing:'Shearing → Scouring → Carding → Spinning → Weaving',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Wool'}] },
   { name:'Leather', type:'processed', category:'Organic',
    components:['Animal hide'], sources_natural:['Cattle, sheep, goat hides (byproduct of meat industry)'],
    properties:{ density:'0.86 g/cm³', tensile_strength:'10-30 MPa', thickness:'0.5-3.0 mm' },
    uses:['Shoes','Bags','Furniture','Belts','Jackets'],
    processing:'Hide → Liming → Dehairing → Tanning (chrome/vegetable) → Drying → Finishing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Leather'}] },
   { name:'Rubber (Natural)', type:'raw', category:'Organic',
    components:[], sources_natural:['Rubber tree plantations in Thailand, Indonesia, Malaysia'],
    properties:{ density:'0.92 g/cm³', tensile_strength:'20-30 MPa', elongation:'600-800%', elasticity:'Very high' },
    uses:['Tires','Gloves','Hoses','Footwear','Elastic bands'],
    processing:'Tapping latex → Coagulation → Smoking/Drying → Vulcanization',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Natural_rubber'}] },
   { name:'Paper', type:'processed', category:'Organic',
    components:['Wood pulp'], sources_natural:['Managed forests, recycled paper'],
    properties:{ density:'0.7-1.2 g/cm³', tensile_strength:'2-10 kN/m', thickness:'0.05-0.5 mm' },
    uses:['Writing','Packaging','Printing','Tissue','Currency'],
    processing:'Wood → Chipping → Pulping → Bleaching → Pressing → Drying → Paper',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Paper'}] },
   { name:'Bamboo', type:'raw', category:'Organic',
    components:[], sources_natural:['Tropical and subtropical forests in Asia, South America'],
    properties:{ density:'0.3-0.8 g/cm³', tensile_strength:'140-280 MPa', growth_rate:'Up to 91 cm/day' },
    uses:['Construction','Flooring','Textiles','Furniture','Scaffolding'],
    processing:'Harvesting → Splitting → Treating → Drying → Laminating',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Bamboo'}] },
   { name:'Hemp', type:'raw', category:'Organic',
    components:[], sources_natural:['Hemp farms in China, Canada, Europe'],
    properties:{ density:'1.48 g/cm³', tensile_strength:'550-900 MPa', fiber_length:'1-5 m' },
    uses:['Rope','Textiles','Paper','Insulation','Bioplastics'],
    processing:'Harvesting → Retting → Breaking → Scutching → Hackling → Spinning',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Hemp'}] },
   { name:'Cork', type:'raw', category:'Organic',
    components:[], sources_natural:['Cork oak forests in Portugal, Spain'],
    properties:{ density:'0.12-0.24 g/cm³', compressibility:'Very high', thermal_conductivity:'0.04 W/m·K' },
    uses:['Wine stoppers','Flooring','Insulation','Bulletin boards','Gaskets'],
    processing:'Bark stripping (every 9 years) → Boiling → Drying → Cutting → Finishing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Cork_(material)'}] },
   { name:'Granite', type:'raw', category:'Mineral',
    components:[], sources_natural:['Quarries worldwide — India, Brazil, Norway, USA'],
    properties:{ density:'2.63-2.75 g/cm³', compressive_strength:'100-250 MPa', hardness:'6-7 Mohs' },
    uses:['Countertops','Building facades','Monuments','Paving'],
    processing:'Quarrying → Diamond wire cutting → Polishing → Finishing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Granite'}] },
   { name:'Marble', type:'raw', category:'Mineral',
    components:[], sources_natural:['Quarries in Italy (Carrara), Greece, Turkey, India'],
    properties:{ density:'2.56 g/cm³', compressive_strength:'60-170 MPa', hardness:'3-4 Mohs' },
    uses:['Sculpture','Countertops','Flooring','Building facades'],
    processing:'Quarrying → Block cutting → Gang sawing → Polishing → Finishing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Marble'}] },
   { name:'Limestone', type:'raw', category:'Mineral',
    components:[], sources_natural:['Sedimentary deposits worldwide'],
    properties:{ density:'2.3-2.7 g/cm³', composition:'Primarily CaCO₃', hardness:'3-4 Mohs' },
    uses:['Cement production','Building stone','Agriculture (pH adjustment)','Steel flux'],
    processing:'Quarrying → Crushing → Calcination (for quicklime) → Hydration (for slaked lime)',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/crushed-stone-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Limestone'}] },
   { name:'Cement', type:'processed', category:'Mineral',
    components:['Limestone','Clay','Gypsum'], sources_natural:['Limestone and clay quarries'],
    properties:{ density:'1.5 g/cm³ (powder)', setting_time:'30-90 minutes initial', compressive_strength:'20-60 MPa (with water)' },
    uses:['Concrete production','Mortar','Grout','Stucco'],
    processing:'Limestone + Clay → Kiln (1450°C) → Clinker → Grinding with gypsum → Cement',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center/cement-statistics-and-information'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Cement'}] },
   { name:'Brick', type:'processed', category:'Mineral',
    components:['Clay','Shale'], sources_natural:['Clay pits and shale quarries'],
    properties:{ density:'1.8-2.0 g/cm³', compressive_strength:'10-35 MPa', water_absorption:'5-20%' },
    uses:['Walls','Paving','Chimneys','Fireplaces','Landscaping'],
    processing:'Clay → Mixing → Molding/Extrusion → Drying → Kiln firing (900-1100°C) → Brick',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Brick'}] },
   { name:'Porcelain', type:'processed', category:'Mineral',
    components:['Kaolin clay','Feldspar','Quartz'], sources_natural:['Kaolin deposits in China, UK, USA'],
    properties:{ density:'2.3-2.5 g/cm³', hardness:'7 Mohs', water_absorption:'<0.5%', firing_temp:'1260-1400 °C' },
    uses:['Dinnerware','Tiles','Electrical insulators','Dental crowns','Bathroom fixtures'],
    processing:'Kaolin + Feldspar + Quartz → Ball milling → Shaping → Bisque firing → Glazing → High firing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Porcelain'}] },
   { name:'Fiberglass', type:'composite', category:'Mineral',
    components:['Glass fibers','Polymer resin'], sources_natural:['Sand (for glass) + petrochemicals (for resin)'],
    properties:{ density:'1.5-2.0 g/cm³', tensile_strength:'1000-3500 MPa', thermal_conductivity:'Low' },
    uses:['Insulation','Boats','Car bodies','Tanks','Surfboards'],
    processing:'Glass melting → Fiber drawing → Resin impregnation → Layup → Curing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Fiberglass'}] },
   { name:'Plastic (Polyethylene)', type:'processed', category:'Synthetic',
    components:['Ethylene (from petroleum)'], sources_natural:['Petroleum refineries'],
    properties:{ density:'0.91-0.97 g/cm³', tensile_strength:'8-33 MPa', melting_point:'115-135 °C' },
    uses:['Packaging','Bottles','Bags','Pipes','Toys'],
    processing:'Crude oil → Cracking → Ethylene → Polymerization → Polyethylene pellets → Molding',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Polyethylene'}] },
   { name:'PVC (Polyvinyl Chloride)', type:'processed', category:'Synthetic',
    components:['Vinyl chloride (from ethylene + chlorine)'], sources_natural:['Petroleum + salt (NaCl)'],
    properties:{ density:'1.3-1.45 g/cm³', tensile_strength:'40-60 MPa', melting_point:'100-260 °C' },
    uses:['Pipes','Window frames','Flooring','Cable insulation','Medical tubing'],
    processing:'Ethylene + Chlorine → Vinyl chloride → Polymerization → PVC resin → Extrusion/Molding',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Polyvinyl_chloride'}] },
   { name:'Nylon', type:'processed', category:'Synthetic',
    components:['Adipic acid','Hexamethylenediamine'], sources_natural:['Petroleum-derived chemicals'],
    properties:{ density:'1.13-1.15 g/cm³', tensile_strength:'70-85 MPa', melting_point:'220-260 °C' },
    uses:['Stockings','Rope','Gears','Carpet','Parachutes','Toothbrush bristles'],
    processing:'Petrochemicals → Polymerization → Nylon chips → Melt spinning → Nylon fiber',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Nylon'}] },
   { name:'Carbon Fiber', type:'composite', category:'Synthetic',
    components:['Polyacrylonitrile (PAN)','Epoxy resin'], sources_natural:['Petroleum-derived PAN precursor'],
    properties:{ density:'1.55-1.6 g/cm³', tensile_strength:'3500-7000 MPa', modulus:'230-540 GPa' },
    uses:['Aerospace','Racing cars','Sporting goods','Wind turbine blades','Prosthetics'],
    processing:'PAN fiber → Oxidation (200-300°C) → Carbonization (1000-3000°C) → Surface treatment → Resin layup',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Carbon_fiber_reinforced_polymer'}] },
   { name:'Kevlar', type:'processed', category:'Synthetic',
    components:['Para-phenylenediamine','Terephthaloyl chloride'], sources_natural:['Petroleum-derived chemicals'],
    properties:{ density:'1.44 g/cm³', tensile_strength:'3620 MPa', modulus:'112 GPa', heat_resistance:'Decomposes ~450 °C' },
    uses:['Body armor','Tires','Brake pads','Ropes','Helmets'],
    processing:'Condensation polymerization → Wet spinning → Drawing → Kevlar fiber',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Kevlar'}] },
   { name:'Plywood', type:'composite', category:'Organic',
    components:['Wood veneers','Adhesive'], sources_natural:['Managed forests (softwood and hardwood)'],
    properties:{ density:'0.4-0.7 g/cm³', tensile_strength:'20-70 MPa', thickness:'3-25 mm standard' },
    uses:['Furniture','Subfloors','Wall sheathing','Boats','Formwork'],
    processing:'Logs → Peeling (rotary lathe) → Veneer drying → Gluing → Cross-lamination → Hot pressing',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Plywood'}] },
   { name:'MDF (Medium-Density Fiberboard)', type:'composite', category:'Organic',
    components:['Wood fibers','Urea-formaldehyde resin'], sources_natural:['Sawmill residues, recycled wood'],
    properties:{ density:'0.6-0.8 g/cm³', tensile_strength:'0.5-1.0 MPa (internal bond)', surface:'Very smooth' },
    uses:['Furniture','Cabinets','Molding','Speaker boxes','Shelving'],
    processing:'Wood chipping → Defibration → Resin blending → Mat forming → Hot pressing → Sanding',
    references:[{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Medium-density_fibreboard'}] },
   { name:'Coal', type:'raw', category:'Energy',
    components:[], sources_natural:['Coal mines in China, India, USA, Indonesia, Australia'],
    properties:{ energy_content:'24-35 MJ/kg', carbon_content:'60-95%', density:'1.1-1.5 g/cm³' },
    uses:['Electricity generation','Steel production (coking coal)','Cement','Chemical feedstock'],
    processing:'Mining (surface or underground) → Washing → Crushing → Screening → Coal',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Coal'}] },
   { name:'Petroleum (Crude Oil)', type:'raw', category:'Energy',
    components:[], sources_natural:['Oil fields in Saudi Arabia, USA, Russia, Canada, Iraq'],
    properties:{ energy_content:'42-47 MJ/kg', density:'0.82-0.95 g/cm³ (API 10-45°)', viscosity:'Varies widely' },
    uses:['Gasoline','Diesel','Jet fuel','Plastics','Lubricants','Asphalt'],
    processing:'Extraction → Desalting → Fractional distillation → Cracking → Refining → Products',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Petroleum'}] },
   { name:'Natural Gas', type:'raw', category:'Energy',
    components:[], sources_natural:['Gas fields in USA, Russia, Iran, Qatar, Canada'],
    properties:{ energy_content:'38-50 MJ/m³', composition:'70-90% Methane', density:'0.0007-0.0009 g/cm³' },
    uses:['Electricity generation','Heating','Cooking','Fertilizer (ammonia)','Hydrogen production'],
    processing:'Extraction → Separation → Dehydration → Sweetening (H₂S removal) → Compression/Liquefaction',
    references:[{name:'USGS',url:'https://usgs.gov/centers/national-minerals-information-center'},{name:'Wikipedia',url:'https://en.wikipedia.org/wiki/Natural_gas'}] },
  ];

  // Merge world data into MATERIALS
  MATERIALS.forEach(m => {
   const wd = MATERIAL_WORLD_DATA[m.name];
   if (wd) Object.assign(m, { worldData: wd });
  });

  let materialTypeFilter = '';
  let selectedMaterial = null;

  function renderMaterialPills() {
   const types = ['raw','processed','composite'];
   document.getElementById('material-type-pills').innerHTML =
    `<span class="filter-pill ${materialTypeFilter===''?'active':''}" onclick="materialTypeFilter='';renderMaterials()">All</span>` +
    types.map(t => `<span class="filter-pill ${materialTypeFilter===t?'active':''}" onclick="materialTypeFilter='${t}';renderMaterials()"><span class="material-type-badge mat-${t}">${t}</span></span>`).join('');
  }

  function renderMaterials() {
   renderMaterialPills();
   const filtered = MATERIALS.filter(m => !materialTypeFilter || m.type === materialTypeFilter);
   document.getElementById('materials-list').innerHTML = filtered.map(m => {
    const chain = m.processing.split('→').map(s => s.trim());
    const chainHtml = `<div class="processing-chain">${chain.map((s,i) => (i>0?'<span class="chain-arrow">→</span>':'')+`<span>${s}</span>`).join('')}</div>`;
    return `<div class="material-card" onclick="showMaterialDetail('${m.name.replace(/'/g,"\\'")}')">
     <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.3rem;">
      <span style="font-weight:600;font-size:0.88rem;color:var(--text);">${m.name}</span>
      <span class="material-type-badge mat-${m.type}">${m.type}</span>
     </div>
     <div style="font-size:0.72rem;color:var(--text-muted);margin-bottom:0.3rem;">${m.category}${m.components.length?' · From: '+m.components.join(', '):''}</div>
     ${chainHtml}
    </div>`;
   }).join('');
  }

  function showMaterialDetail(name) {
   const m = MATERIALS.find(x => x.name === name);
   if (!m) return;
   if (selectedMaterial === name) { selectedMaterial = null; document.getElementById('material-detail-slot').innerHTML = ''; return; }
   selectedMaterial = name;
   const propsHtml = Object.entries(m.properties).map(([k,v]) => `<div class="el-prop"><dt>${k.replace(/_/g,' ')}</dt><dd>${v}</dd></div>`).join('');
   const srcLinks = m.references.map(r => `<a href="${r.url}" target="_blank" rel="noopener" style="color:var(--accent);font-size:0.75rem;">${r.name} ↗</a>`).join(' ');
   document.getElementById('material-detail-slot').innerHTML = `
    <div class="element-detail">
     <div style="display:flex;justify-content:space-between;align-items:start;">
      <h3>${m.name} <span class="material-type-badge mat-${m.type}" style="vertical-align:middle;">${m.type}</span></h3>
      <button onclick="selectedMaterial=null;document.getElementById('material-detail-slot').innerHTML=''" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:1.1rem;">Close</button>
     </div>
     <div class="el-props">${propsHtml}</div>
     <div style="margin:0.6rem 0;"><strong style="font-size:0.75rem;color:var(--text-muted);">USES:</strong>
      <div style="display:flex;flex-wrap:wrap;gap:0.3rem;margin-top:0.3rem;">
       ${m.uses.map(u => `<span style="background:rgba(255,136,17,0.1);color:var(--accent);font-size:0.7rem;padding:0.15rem 0.5rem;border-radius:10px;">${u}</span>`).join('')}
      </div>
     </div>
     <div style="margin:0.6rem 0;"><strong style="font-size:0.75rem;color:var(--text-muted);">PROCESSING:</strong>
      <div class="processing-chain" style="margin-top:0.3rem;font-size:0.8rem;">${m.processing.split('→').map((s,i) => (i>0?'<span class="chain-arrow">→</span>':'')+`<span>${s.trim()}</span>`).join('')}</div>
     </div>
     ${m.worldData ? `
     <details style="margin-top:0.6rem;border:1px solid var(--border);border-radius:8px;padding:0.4rem 0.6rem;background:rgba(0,255,136,0.03);">
      <summary style="cursor:pointer;font-size:0.8rem;font-weight:600;color:var(--accent);">🌍 World Data</summary>
      <div style="margin-top:0.4rem;font-size:0.75rem;line-height:1.6;">
       ${m.worldData.production ? '<div><strong>Annual Production:</strong> '+m.worldData.production.annual+(m.worldData.production.growth?' ('+m.worldData.production.growth+')':'')+'</div>' : ''}
       ${m.worldData.inputChain ? '<div><strong>Input Chain:</strong><ul style="margin:0.2rem 0 0.2rem 1.2rem;padding:0;">'+m.worldData.inputChain.map(ic => '<li>'+ic.material+': '+ic.ratio+'</li>').join('')+'</ul></div>' : ''}
       ${m.worldData.energyCost ? '<div><strong>Energy Cost:</strong> '+m.worldData.energyCost+'</div>' : ''}
       ${m.worldData.topProducers ? '<div><strong>Top Producers:</strong> '+m.worldData.topProducers.join(', ')+'</div>' : ''}
       ${m.worldData.co2 ? '<div><strong>CO₂ Footprint:</strong> '+m.worldData.co2+'</div>' : ''}
      </div>
     </details>` : ''}
     <div style="margin-top:0.5rem;">${srcLinks}</div>
    </div>`;
  }
  renderMaterials();

  // ══════════════════════════════════════
  // WORLD DASHBOARD — Chart & Facts
  // ══════════════════════════════════════
  (function initWorldDash() {
   // Bar chart — top resources by production value
   const chartData = [
    {label:'Oil',value:3200,color:'#444'},
    {label:'Coal',value:900,color:'#666'},
    {label:'Iron',value:300,color:'#b44'},
    {label:'Gold',value:215,color:'#da0'},
    {label:'Copper',value:187,color:'#c73'},
    {label:'Al',value:166,color:'#888'},
    {label:'Gas',value:150,color:'#69c'},
    {label:'Ni',value:58,color:'#5a5'},
    {label:'Zn',value:35,color:'#77a'},
    {label:'Li',value:22,color:'#c6f'},
   ];
   const canvas = document.getElementById('world-dash-chart');
   if (canvas && canvas.getContext) {
    const ctx = canvas.getContext('2d');
    const W = canvas.width, H = canvas.height;
    const maxVal = Math.max(...chartData.map(d=>d.value));
    const barW = (W - 80) / chartData.length;
    const topPad = 25, bottomPad = 35, leftPad = 45;
    ctx.fillStyle = 'rgba(255,255,255,0.06)';
    ctx.fillRect(0,0,W,H);
    ctx.fillStyle = '#aaa'; ctx.font = '11px system-ui';
    ctx.fillText('Top Resources by Annual Value ($B)', leftPad, 14);
    chartData.forEach((d,i) => {
     const barH = (d.value / maxVal) * (H - topPad - bottomPad);
     const x = leftPad + i * barW + 4;
     const y = H - bottomPad - barH;
     ctx.fillStyle = d.color;
     ctx.fillRect(x, y, barW - 8, barH);
     ctx.fillStyle = '#ccc'; ctx.font = '9px system-ui';
     ctx.save(); ctx.translate(x + barW/2 - 4, H - 5);
     ctx.rotate(-0.5); ctx.fillText(d.label, 0, 0); ctx.restore();
     ctx.fillStyle = '#eee'; ctx.font = '9px system-ui';
     ctx.fillText('$'+d.value+'B', x + 2, y - 3);
    });
   }
   // Rotating facts
   const facts = [
    '💡 Earth produces enough food for 10+ billion people — waste is the problem, not production.',
    '⛏️ Most "scarce" resources have 50-200+ years of known reserves at current consumption.',
    '☀️ Solar electricity is now cheaper than coal in most markets worldwide.',
    '📉 Global extreme poverty dropped from 36% to 9% in 25 years.',
    '📚 Global literacy has risen from 12% (1820) to 87% today.',
    '👶 Child mortality has dropped 60% since 1990.',
    '🔋 Global battery production tripled in 3 years (2021-2024).',
    '🌊 Earth has 1.4 billion km³ of water — desalination costs have dropped 70% since 2000.',
    '🏗️ Humanity produces 4.1 billion tonnes of cement every year — enough for 33B tonnes of concrete.',
    'âš¡ A single uranium fuel pellet (7g) contains as much energy as 1 tonne of coal.',
    '🌍 The world\'s proven oil reserves today are larger than they were 40 years ago despite consumption.',
    '🚀 Humanity launches 200+ rockets per year — double the rate of a decade ago.',
   ];
   const factEl = document.getElementById('world-dash-fact');
   if (factEl) {
    let idx = Math.floor(Math.random() * facts.length);
    factEl.textContent = facts[idx];
    setInterval(() => { idx = (idx + 1) % facts.length; factEl.textContent = facts[idx]; }, 12000);
   }
  })();

  // ══════════════════════════════════════
  // PERSONAL INVENTORY
  // ══════════════════════════════════════
  const INV_CATEGORIES = [
   {id:'Vehicle',icon:'🚗'},{id:'Clothing',icon:'👕'},{id:'Electronics',icon:'💻'},{id:'Tools',icon:'🔧'},
   {id:'Furniture',icon:'🪑'},{id:'Kitchen',icon:'🍳'},{id:'Books/Media',icon:'📚'},{id:'Gaming',icon:'🎮'},
   {id:'Home',icon:'🏠'},{id:'Other',icon:'📦'}
  ];

  (function populateInvCategories() {
   const sel = document.getElementById('inv-category');
   const filterSel = document.getElementById('inv-category-filter');
   INV_CATEGORIES.forEach(c => {
    sel.innerHTML += `<option value="${c.id}">${c.icon} ${c.id}</option>`;
    filterSel.innerHTML += `<option value="${c.id}">${c.icon} ${c.id}</option>`;
   });
  })();

  function loadInventory() { try { return JSON.parse(localStorage.getItem('humanity_inventory')) || []; } catch { return []; } }
  function saveInventory(items) { localStorage.setItem('humanity_inventory', JSON.stringify(items)); }

  function renderInventory() {
   const items = loadInventory();
   const catFilter = document.getElementById('inv-category-filter').value;
   const search = (document.getElementById('inv-search').value || '').toLowerCase();
   const filtered = items.filter(item => {
    if (catFilter && item.category !== catFilter) return false;
    if (search && !item.name.toLowerCase().includes(search) && !(item.tags||[]).some(t => t.toLowerCase().includes(search)) && !(item.category||'').toLowerCase().includes(search)) return false;
    return true;
   });
   const catIcon = (id) => (INV_CATEGORIES.find(c => c.id === id) || {icon:'📦'}).icon;
   document.getElementById('inventory-list').innerHTML = filtered.length === 0
    ? '<div style="color:var(--text-muted);font-size:0.82rem;font-style:italic;padding:1rem;text-align:center;grid-column:1/-1;">No items yet — click "+ Add Item" to start</div>'
    : filtered.map(item => `
     <div class="inv-item">
      <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.3rem;">
       <span style="font-weight:600;font-size:0.88rem;">${catIcon(item.category)} ${escHtml(item.name)}</span>
       <div style="display:flex;gap:0.3rem;">
        <button onclick="listInventoryForSale('${item.id}')" style="background:none;border:none;color:var(--accent);cursor:pointer;font-size:0.65rem;" title="List for Sale">📢</button>
        <button onclick="editInventoryItem('${item.id}')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.75rem;" title="Edit">✏️</button>
        <button onclick="deleteInventoryItem('${item.id}')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.75rem;" title="Delete">🗑️</button>
       </div>
      </div>
      <div style="font-size:0.72rem;color:var(--text-muted);">
       ${item.category}${item.subcategory?' · '+escHtml(item.subcategory):''} · Qty: ${item.quantity||1} · ${item.condition||'Good'}
      </div>
      ${item.location?`<div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.15rem;">📍 ${escHtml(item.location)}</div>`:''}
      ${item.description?`<div style="font-size:0.75rem;color:var(--text);margin-top:0.3rem;">${escHtml(item.description)}</div>`:''}
      ${(item.tags||[]).length?`<div style="display:flex;flex-wrap:wrap;gap:0.2rem;margin-top:0.3rem;">${item.tags.map(t=>`<span style="background:rgba(255,255,255,0.05);font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:8px;color:var(--text-muted);">#${escHtml(t)}</span>`).join('')}</div>`:''}
     </div>`).join('');
  }

  function openInventoryModal(editId) {
   document.getElementById('inv-modal-title').textContent = editId ? 'Edit Item' : 'Add Item';
   document.getElementById('inv-edit-id').value = editId || '';
   if (editId) {
    const item = loadInventory().find(i => i.id === editId);
    if (!item) return;
    document.getElementById('inv-name').value = item.name || '';
    document.getElementById('inv-category').value = item.category || 'Other';
    document.getElementById('inv-subcategory').value = item.subcategory || '';
    document.getElementById('inv-description').value = item.description || '';
    document.getElementById('inv-quantity').value = item.quantity || 1;
    document.getElementById('inv-condition').value = item.condition || 'Good';
    document.getElementById('inv-acquired').value = item.acquired || '';
    document.getElementById('inv-location').value = item.location || '';
    document.getElementById('inv-value').value = item.value || '';
    document.getElementById('inv-notes').value = item.notes || '';
    document.getElementById('inv-tags').value = (item.tags||[]).join(', ');
   } else {
    ['inv-name','inv-subcategory','inv-description','inv-acquired','inv-location','inv-value','inv-notes','inv-tags'].forEach(id => document.getElementById(id).value = '');
    document.getElementById('inv-quantity').value = 1;
    document.getElementById('inv-condition').value = 'Good';
    document.getElementById('inv-category').value = 'Other';
   }
   document.getElementById('inv-modal').classList.add('open');
  }
  function closeInventoryModal() { document.getElementById('inv-modal').classList.remove('open'); }
  function editInventoryItem(id) { openInventoryModal(id); }
  function deleteInventoryItem(id) {
   if (!confirm('Delete this item?')) return;
   saveInventory(loadInventory().filter(i => i.id !== id));
   renderInventory();
  }
  function saveInventoryItem() {
   const name = document.getElementById('inv-name').value.trim();
   if (!name) { document.getElementById('inv-name').style.borderColor = '#e55'; return; }
   const editId = document.getElementById('inv-edit-id').value;
   const items = loadInventory();
   const item = {
    id: editId || Date.now().toString(36) + Math.random().toString(36).slice(2,6),
    name,
    category: document.getElementById('inv-category').value,
    subcategory: document.getElementById('inv-subcategory').value.trim(),
    description: document.getElementById('inv-description').value.trim(),
    quantity: parseInt(document.getElementById('inv-quantity').value) || 1,
    condition: document.getElementById('inv-condition').value,
    acquired: document.getElementById('inv-acquired').value.trim(),
    location: document.getElementById('inv-location').value.trim(),
    value: document.getElementById('inv-value').value.trim(),
    notes: document.getElementById('inv-notes').value.trim(),
    tags: document.getElementById('inv-tags').value.split(',').map(t => t.trim()).filter(Boolean),
   };
   if (editId) {
    const idx = items.findIndex(i => i.id === editId);
    if (idx >= 0) items[idx] = item; else items.push(item);
   } else {
    items.push(item);
   }
   saveInventory(items);
   closeInventoryModal();
   renderInventory();
  }

  function exportInventory() {
   const data = JSON.stringify(loadInventory(), null, 2);
   const blob = new Blob([data], {type:'application/json'});
   const a = document.createElement('a');
   a.href = URL.createObjectURL(blob);
   a.download = 'humanity-inventory-' + new Date().toISOString().split('T')[0] + '.json';
   a.click();
  }
  function importInventory(event) {
   const file = event.target.files[0];
   if (!file) return;
   const reader = new FileReader();
   reader.onload = (e) => {
    try {
     const imported = JSON.parse(e.target.result);
     if (!Array.isArray(imported)) throw new Error('Not an array');
     const existing = loadInventory();
     const merged = [...existing, ...imported.map(i => ({...i, id: i.id || Date.now().toString(36)+Math.random().toString(36).slice(2,6)}))];
     saveInventory(merged);
     renderInventory();
     alert('Imported ' + imported.length + ' items.');
    } catch(err) { alert('Import failed: ' + err.message); }
   };
   reader.readAsText(file);
   event.target.value = '';
  }

  // Close inv modal on backdrop
  document.getElementById('inv-modal').addEventListener('click', function(e) { if (e.target === this) closeInventoryModal(); });

  renderInventory();

  // ══════════════════════════════════════
  // ASSET LIBRARY
  // ══════════════════════════════════════

  let assetLibrary = [];
  let assetCategoryFilter = '';
  let assetViewMode = 'grid'; // 'grid' or 'list'
  let currentPreviewAsset = null;

  function loadAssetLibrary() {
   try { return JSON.parse(localStorage.getItem('humanity_assets')) || []; } catch { return []; }
  }
  function saveAssetLibrary(assets) {
   localStorage.setItem('humanity_assets', JSON.stringify(assets));
  }

  assetLibrary = loadAssetLibrary();

  // Drag-and-drop
  const dropzone = document.getElementById('asset-dropzone');
  if (dropzone) {
   dropzone.addEventListener('dragover', (e) => { e.preventDefault(); dropzone.style.borderColor = 'var(--accent)'; dropzone.style.background = 'rgba(255,136,17,0.05)'; });
   dropzone.addEventListener('dragleave', () => { dropzone.style.borderColor = 'var(--border)'; dropzone.style.background = ''; });
   dropzone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropzone.style.borderColor = 'var(--border)';
    dropzone.style.background = '';
    if (e.dataTransfer.files.length) handleAssetFiles(e.dataTransfer.files);
   });
  }

  function categorizeFile(filename) {
   const ext = filename.split('.').pop().toLowerCase();
   if (['png','jpg','jpeg','gif','webp','svg'].includes(ext)) return 'image';
   if (['blend','stl','obj','gltf','glb'].includes(ext)) return '3d_model';
   if (['mp3','wav','ogg'].includes(ext)) return 'audio';
   if (['pdf','txt','md'].includes(ext)) return 'document';
   return 'document';
  }

  function fileTypeIcon(category) {
   const icons = { image: '🖼️', '3d_model': '🧊', audio: '🎵', document: '📄' };
   return icons[category] || '📎';
  }

  function formatFileSize(bytes) {
   if (bytes < 1024) return bytes + ' B';
   if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
   return (bytes / 1048576).toFixed(1) + ' MB';
  }

  async function handleAssetFiles(files) {
   const token = localStorage.getItem('humanity_upload_token');
   const key = localStorage.getItem('humanity_key');
   if (!token && !key) { alert('You must be connected to upload files.'); return; }

   const progress = document.getElementById('asset-upload-progress');
   const bar = document.getElementById('asset-upload-bar');
   const status = document.getElementById('asset-upload-status');
   progress.style.display = '';

   for (let i = 0; i < files.length; i++) {
    const file = files[i];
    bar.style.width = ((i / files.length) * 100) + '%';
    status.textContent = `Uploading ${file.name} (${i + 1}/${files.length})…`;

    try {
     const form = new FormData();
     form.append('file', file, file.name);
     const params = token ? `?token=${encodeURIComponent(token)}` : `?key=${encodeURIComponent(key)}`;
     const res = await fetch('/api/upload' + params, { method: 'POST', body: form });
     if (!res.ok) { const err = await res.text(); throw new Error(err); }
     const data = await res.json();

     const category = categorizeFile(file.name);
     const asset = {
      id: Date.now().toString(36) + Math.random().toString(36).slice(2, 8),
      filename: file.name,
      url: data.url,
      type: data.type || category,
      category: category,
      tags: [],
      size: file.size,
      uploadedAt: new Date().toISOString(),
      description: '',
     };

     assetLibrary.unshift(asset);
     saveAssetLibrary(assetLibrary);

     // Also create server-side record
     try {
      await fetch('/api/assets', {
       method: 'POST',
       headers: { 'Content-Type': 'application/json' },
       body: JSON.stringify({
        filename: asset.filename,
        url: asset.url,
        file_type: asset.type,
        category: asset.category,
        tags: asset.tags,
        size_bytes: asset.size,
        description: asset.description,
        owner_key: key,
       }),
      });
     } catch (e) { console.warn('Failed to create server asset record:', e); }
    } catch (err) {
     console.error('Upload failed:', err);
     status.textContent = `Failed: ${file.name} — ${err.message}`;
     await new Promise(r => setTimeout(r, 2000));
    }
   }

   bar.style.width = '100%';
   status.textContent = 'Done!';
   setTimeout(() => { progress.style.display = 'none'; bar.style.width = '0%'; }, 1500);
   renderAssets();
   updateAssetTagFilter();
  }

  function setAssetCategory(cat) {
   assetCategoryFilter = cat;
   document.querySelectorAll('[id^="asset-cat-"]').forEach(el => el.classList.remove('active'));
   document.getElementById('asset-cat-' + (cat || 'all')).classList.add('active');
   renderAssets();
  }

  function toggleAssetView() {
   assetViewMode = assetViewMode === 'grid' ? 'list' : 'grid';
   const btn = document.getElementById('asset-view-toggle');
   btn.textContent = assetViewMode === 'grid' ? '☰' : '▦';
   renderAssets();
  }

  function renderAssets() {
   const search = (document.getElementById('asset-search').value || '').toLowerCase();
   const tagFilter = document.getElementById('asset-tag-filter').value;

   let filtered = assetLibrary.filter(a => {
    if (assetCategoryFilter && a.category !== assetCategoryFilter) return false;
    if (tagFilter && !(a.tags || []).includes(tagFilter)) return false;
    if (search && !a.filename.toLowerCase().includes(search) && !(a.tags || []).some(t => t.toLowerCase().includes(search)) && !(a.description || '').toLowerCase().includes(search)) return false;
    return true;
   });

   const grid = document.getElementById('asset-grid');
   const empty = document.getElementById('asset-empty');

   if (filtered.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
    return;
   }
   empty.style.display = 'none';

   if (assetViewMode === 'grid') {
    grid.style.gridTemplateColumns = 'repeat(auto-fill,minmax(180px,1fr))';
    grid.innerHTML = filtered.map(a => {
     const icon = fileTypeIcon(a.category);
     const thumb = a.category === 'image' ? `<div style="height:100px;background:url('${a.url}') center/cover no-repeat;border-radius:6px 6px 0 0;"></div>` :
      `<div style="height:100px;display:flex;align-items:center;justify-content:center;background:var(--bg-panel);border-radius:6px 6px 0 0;font-size:2rem;">${icon}</div>`;
     return `<div onclick="previewAsset('${a.id}')" style="background:var(--bg-card);border:1px solid var(--border);border-radius:6px;overflow:hidden;cursor:pointer;transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'">
      ${thumb}
      <div style="padding:0.5rem;">
       <div style="font-size:0.78rem;font-weight:600;color:var(--text);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;" title="${escHtml(a.filename)}">${escHtml(a.filename)}</div>
       <div style="font-size:0.65rem;color:var(--text-muted);display:flex;justify-content:space-between;">
        <span>${formatFileSize(a.size)}</span>
        <span>${a.uploadedAt ? new Date(a.uploadedAt).toLocaleDateString() : ''}</span>
       </div>
       ${(a.tags || []).length ? `<div style="display:flex;gap:0.2rem;flex-wrap:wrap;margin-top:0.3rem;">${a.tags.slice(0, 3).map(t => `<span style="font-size:0.55rem;background:rgba(255,136,17,0.15);color:var(--accent);padding:0.1rem 0.3rem;border-radius:3px;">${escHtml(t)}</span>`).join('')}</div>` : ''}
      </div>
     </div>`;
    }).join('');
   } else {
    grid.style.gridTemplateColumns = '1fr';
    grid.innerHTML = filtered.map(a => {
     const icon = fileTypeIcon(a.category);
     return `<div onclick="previewAsset('${a.id}')" style="display:flex;align-items:center;gap:0.6rem;padding:0.5rem;background:var(--bg-card);border:1px solid var(--border);border-radius:6px;cursor:pointer;transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'">
      <span style="font-size:1.3rem;">${icon}</span>
      <div style="flex:1;min-width:0;">
       <div style="font-size:0.8rem;font-weight:600;color:var(--text);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${escHtml(a.filename)}</div>
       <div style="font-size:0.65rem;color:var(--text-muted);">${formatFileSize(a.size)} · ${a.uploadedAt ? new Date(a.uploadedAt).toLocaleDateString() : ''}</div>
      </div>
      <div style="display:flex;gap:0.2rem;">${(a.tags || []).slice(0, 2).map(t => `<span style="font-size:0.55rem;background:rgba(255,136,17,0.15);color:var(--accent);padding:0.1rem 0.3rem;border-radius:3px;">${escHtml(t)}</span>`).join('')}</div>
     </div>`;
    }).join('');
   }
  }

  function previewAsset(id) {
   const asset = assetLibrary.find(a => a.id === id);
   if (!asset) return;
   currentPreviewAsset = asset;

   document.getElementById('asset-preview-title').textContent = asset.filename;

   // Build preview content
   let content = '';
   if (asset.category === 'image') {
    content = `<img src="${asset.url}" style="max-width:100%;max-height:400px;border-radius:8px;display:block;margin:0 auto;">`;
   } else if (asset.category === 'audio') {
    content = `<audio controls src="${asset.url}" style="width:100%;"></audio>`;
   } else if (asset.category === '3d_model') {
    const ext = asset.filename.split('.').pop().toUpperCase();
    content = `<div style="text-align:center;padding:2rem;background:var(--bg-panel);border-radius:8px;">
     <div style="font-size:3rem;margin-bottom:0.5rem;">🧊</div>
     <div style="font-size:0.9rem;color:var(--text);">${ext} 3D Model</div>
     <div style="font-size:0.8rem;color:var(--text-muted);margin-top:0.3rem;">${formatFileSize(asset.size)}</div>
     <a href="${asset.url}" download="${escHtml(asset.filename)}" style="display:inline-block;margin-top:0.8rem;padding:0.4rem 1rem;background:var(--accent);color:#fff;border-radius:6px;text-decoration:none;font-size:0.8rem;">⬇️ Download</a>
    </div>`;
   } else if (asset.category === 'document') {
    const ext = asset.filename.split('.').pop().toLowerCase();
    if (ext === 'txt' || ext === 'md') {
     content = `<div id="asset-text-preview" style="background:var(--bg-panel);padding:1rem;border-radius:8px;font-size:0.8rem;color:var(--text);max-height:400px;overflow-y:auto;white-space:pre-wrap;font-family:monospace;">Loading…</div>`;
     fetch(asset.url).then(r => r.text()).then(text => {
      const el = document.getElementById('asset-text-preview');
      if (el) el.textContent = text.slice(0, 10000);
     }).catch(() => {});
    } else {
     content = `<div style="text-align:center;padding:2rem;">
      <div style="font-size:3rem;">📄</div>
      <a href="${asset.url}" target="_blank" style="color:var(--accent);font-size:0.85rem;">Open Document</a>
     </div>`;
    }
   }
   document.getElementById('asset-preview-content').innerHTML = content;

   // Meta info
   document.getElementById('asset-preview-meta').innerHTML = `
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.3rem;">
     <div>📁 ${escHtml(asset.category)}</div>
     <div>💾 ${formatFileSize(asset.size)}</div>
     <div>📅 ${asset.uploadedAt ? new Date(asset.uploadedAt).toLocaleString() : 'Unknown'}</div>
     <div>🔗 <a href="${asset.url}" target="_blank" style="color:var(--accent);">Direct Link</a></div>
    </div>
    ${asset.description ? `<div style="margin-top:0.4rem;">📝 ${escHtml(asset.description)}</div>` : ''}
   `;

   // Tags
   document.getElementById('asset-preview-tags').innerHTML = (asset.tags || []).length ?
    `<div style="display:flex;gap:0.3rem;flex-wrap:wrap;">${asset.tags.map(t => `<span style="font-size:0.7rem;background:rgba(255,136,17,0.15);color:var(--accent);padding:0.15rem 0.4rem;border-radius:4px;">${escHtml(t)}</span>`).join('')}</div>` : '';

   document.getElementById('asset-preview-modal').style.display = '';
  }

  function closeAssetPreview() {
   document.getElementById('asset-preview-modal').style.display = 'none';
   currentPreviewAsset = null;
  }

  function editAssetTags() {
   if (!currentPreviewAsset) return;
   document.getElementById('asset-tag-input').value = (currentPreviewAsset.tags || []).join(', ');
   document.getElementById('asset-tag-modal').style.display = '';
  }

  function closeAssetTagModal() {
   document.getElementById('asset-tag-modal').style.display = 'none';
  }

  function saveAssetTags() {
   if (!currentPreviewAsset) return;
   const tags = document.getElementById('asset-tag-input').value.split(',').map(t => t.trim()).filter(Boolean);
   const idx = assetLibrary.findIndex(a => a.id === currentPreviewAsset.id);
   if (idx >= 0) {
    assetLibrary[idx].tags = tags;
    currentPreviewAsset.tags = tags;
    saveAssetLibrary(assetLibrary);
    renderAssets();
    updateAssetTagFilter();
    // Re-render tags in preview
    document.getElementById('asset-preview-tags').innerHTML = tags.length ?
     `<div style="display:flex;gap:0.3rem;flex-wrap:wrap;">${tags.map(t => `<span style="font-size:0.7rem;background:rgba(255,136,17,0.15);color:var(--accent);padding:0.15rem 0.4rem;border-radius:4px;">${escHtml(t)}</span>`).join('')}</div>` : '';
   }
   closeAssetTagModal();
  }

  function shareAsset() {
   if (!currentPreviewAsset) return;
   const url = window.location.origin + currentPreviewAsset.url;
   navigator.clipboard.writeText(url).then(() => {
    alert('Link copied to clipboard!');
   }).catch(() => {
    prompt('Share link:', url);
   });
  }

  function listAssetOnMarket() {
   if (!currentPreviewAsset) return;
   const a = currentPreviewAsset;
   closeAssetPreview();
   switchTab('market');
   showMarketSection('marketplace');

   // Determine category mapping
   const catMap = { 'image': 'Crafts', '3d_model': 'Gaming', 'audio': 'Books/Media', 'document': 'Books/Media' };
   const modelCats = ['Vehicles','Architecture','Characters','Props','Weapons','Nature','Sci-Fi','Other'];
   const category = a.category === '3d_model' ? '3D Models' : (catMap[a.category] || 'Other');

   openListingModal({
    title: a.filename,
    description: (a.description || '') + `\n\nFile: ${a.filename} (${formatFileSize(a.size)})` + (a.category === '3d_model' ? `\nFormat: ${a.filename.split('.').pop().toUpperCase()}` : ''),
    category: category,
    condition: 'N/A',
    price: 'Free (donations welcome)',
    asset_url: a.url,
   });
  }

  function deleteCurrentAsset() {
   if (!currentPreviewAsset) return;
   if (!confirm('Delete this asset?')) return;
   const id = currentPreviewAsset.id;
   assetLibrary = assetLibrary.filter(a => a.id !== id);
   saveAssetLibrary(assetLibrary);
   closeAssetPreview();
   renderAssets();
   updateAssetTagFilter();

   // Also try to delete server-side
   const key = localStorage.getItem('humanity_key');
   const token = localStorage.getItem('humanity_upload_token');
   const params = token ? `?token=${encodeURIComponent(token)}` : (key ? `?key=${encodeURIComponent(key)}` : '');
   fetch(`/api/assets/${id}${params}`, { method: 'DELETE' }).catch(() => {});
  }

  function updateAssetTagFilter() {
   const allTags = new Set();
   assetLibrary.forEach(a => (a.tags || []).forEach(t => allTags.add(t)));
   const sel = document.getElementById('asset-tag-filter');
   const current = sel.value;
   sel.innerHTML = '<option value="">All Tags</option>' + [...allTags].sort().map(t => `<option value="${escHtml(t)}"${t === current ? ' selected' : ''}>${escHtml(t)}</option>`).join('');
  }

  // Include assets in universal search
  const origUniversalSearch = universalSearch;
  universalSearch = function(query) {
   origUniversalSearch(query);
   if (!query || query.length < 2) return;
   const q = query.toLowerCase();
   const assetHits = assetLibrary.filter(a => a.filename.toLowerCase().includes(q) || (a.tags || []).some(t => t.toLowerCase().includes(q)));
   if (assetHits.length > 0) {
    const results = document.getElementById('universal-search-results');
    const existingResults = window.universalSearchResults || [];
    assetHits.slice(0, 5).forEach((a, i) => {
     existingResults.push(() => previewAsset(a.id));
     const div = document.createElement('div');
     div.className = 'catalog-result-item';
     div.onclick = () => previewAsset(a.id);
     div.innerHTML = `<span class="catalog-result-badge" style="background:#ff881122;color:#ff8811;">Asset</span><span style="color:var(--text);">${escHtml(a.filename)}</span>`;
     results.appendChild(div);
    });
    window.universalSearchResults = existingResults;
   }
  };

  renderAssets();
  updateAssetTagFilter();

  // ══════════════════════════════════════
  // MARKETPLACE
  // ══════════════════════════════════════

  const STORE_DIRECTORY = [
   { name: "Amazon", url: "https://amazon.com", icon: "🏪", category: "General", description: "Everything store" },
   { name: "Walmart", url: "https://walmart.com", icon: "🏪", category: "General", description: "Everyday low prices" },
   { name: "Etsy", url: "https://etsy.com", icon: "🎨", category: "Handmade & Vintage", description: "Handmade, vintage, and unique goods" },
   { name: "eBay", url: "https://ebay.com", icon: "🏷️", category: "Auctions & Resale", description: "Buy and sell anything" },
   { name: "Newegg", url: "https://newegg.com", icon: "💻", category: "Electronics", description: "Computer hardware and electronics" },
   { name: "Home Depot", url: "https://homedepot.com", icon: "🔨", category: "Home & Garden", description: "Home improvement and tools" },
   { name: "REI", url: "https://rei.com", icon: "⛺", category: "Outdoors", description: "Outdoor gear and clothing" },
   { name: "Steam", url: "https://store.steampowered.com", icon: "🎮", category: "Gaming", description: "PC games and software" },
   { name: "GOG", url: "https://gog.com", icon: "🎮", category: "Gaming", description: "DRM-free games" },
   { name: "Bandcamp", url: "https://bandcamp.com", icon: "🎵", category: "Music", description: "Independent music" },
   { name: "Ko-fi", url: "https://ko-fi.com", icon: "☕", category: "Creator Support", description: "Support creators directly" },
   { name: "GitHub Sponsors", url: "https://github.com/sponsors", icon: "💝", category: "Creator Support", description: "Fund open source developers" },
  ];

  const LISTING_CATEGORIES = ['Electronics','Vehicles','Clothing','Tools','Furniture','Home','Books/Media','Gaming','Sports','Crafts','Food/Garden','Services','3D Models','Other'];
  const LISTING_CONDITIONS = ['New','Like New','Good','Fair','Poor','N/A'];
  const CATEGORY_COLORS = {Electronics:'#08f',Vehicles:'#f80',Clothing:'#f0a',Tools:'#fa0',Furniture:'#a66',Home:'#4a8','Books/Media':'#84f',Gaming:'#0cf',Sports:'#4c4',Crafts:'#c4a','Food/Garden':'#6a4',Services:'#48f','3D Models':'#a6f',Other:'#888'};

  let marketListings = [];
  let marketWs = null;
  let ws = null; // Alias for streaming code to use
  let marketMyKey = null;
  let marketMyName = null;
  let marketMyRole = '';
  let marketCurrentSection = 'marketplace';

  function showMarketSection(section) {
   marketCurrentSection = section;
   ['marketplace','stores','mylistings'].forEach(s => {
    const el = document.getElementById('market-section-' + s);
    const btn = document.getElementById('market-nav-' + s);
    if (el) el.style.display = s === section ? '' : 'none';
    if (btn) btn.className = s === section ? 'btn btn-clickable' : 'btn';
   });
   const createBtn = document.getElementById('market-create-btn');
   if (createBtn) createBtn.style.display = (section === 'marketplace' && canCreateListing()) ? '' : 'none';
   if (section === 'stores') renderStoreDirectory();
   if (section === 'mylistings') renderMyListings();
   if (section === 'marketplace') renderMarketListings();
  }

  function canCreateListing() {
   return marketMyRole === 'verified' || marketMyRole === 'donor' || marketMyRole === 'mod' || marketMyRole === 'admin';
  }

  function marketConnect() {
   const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
   marketWs = new WebSocket(proto + '//' + location.host + '/ws');
   marketWs.onopen = () => {
    let storedKey = localStorage.getItem('humanity_key');
    // Also check the Ed25519 key backup (set by chat client's crypto.js)
    if (!storedKey) {
     try {
      const backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
      if (backup && backup.publicKeyHex) storedKey = backup.publicKeyHex;
     } catch(e) {}
    }
    const storedName = localStorage.getItem('humanity_name');
    if (storedKey) {
     marketMyKey = storedKey;
     marketMyName = storedName;
     marketWs.send(JSON.stringify({ type: 'identify', public_key: storedKey, display_name: storedName || null }));
    } else {
     marketMyKey = 'viewer_' + Math.random().toString(36).slice(2, 10);
     marketWs.send(JSON.stringify({ type: 'identify', public_key: marketMyKey, display_name: null }));
    }
   };
   ws = marketWs; // Alias for streaming code
   window._humanityWs = marketWs; // Alias for Skill DNA
   marketWs.onmessage = (e) => {
    try {
     const msg = JSON.parse(e.data);
     handleMarketMessage(msg);
     // Route stream messages
     if (msg.type && msg.type.startsWith('stream_')) {
      streamHandleMessage(msg);
     }
     // (stream_viewer_ready via Private/System removed — using direct stream_offer request instead)
     // Handle skill verification messages via Private/system
     if (msg.type === 'private' && msg.message) {
      if (msg.message.startsWith('__skill_verify_req__:')) {
       try {
        const payload = JSON.parse(msg.message.slice('__skill_verify_req__:'.length));
        if (confirm(`${payload.from_name} claims ${payload.skill_id} Lv ${payload.level} — can you verify?\n\nClick OK to verify, Cancel to decline.`)) {
         const note = prompt('Add a note (optional):') || 'Verified';
         window._humanityWs.send(JSON.stringify({ type: 'skill_verify_response', skill_id: payload.skill_id, to_key: payload.from_key, approved: true, note }));
        }
       } catch(e) {}
      }
      if (msg.message.startsWith('__skill_verify_resp__:')) {
       try {
        const payload = JSON.parse(msg.message.slice('__skill_verify_resp__:'.length));
        if (window._sdHandleVerifyResponse) window._sdHandleVerifyResponse(payload);
       } catch(e) {}
      }
     }
    } catch {}
   };
   marketWs.onclose = () => { setTimeout(marketConnect, 3000); };
   marketWs.onerror = () => {};
  }

  function handleMarketMessage(msg) {
   switch (msg.type) {
    case 'peer_list':
     if (msg.peers && marketMyKey) {
      const me = msg.peers.find(p => p.public_key === marketMyKey);
      if (me) {
       marketMyRole = me.role || '';
       marketMyName = me.display_name || marketMyName;
       if (me.upload_token) localStorage.setItem('humanity_upload_token', me.upload_token);
      }
     }
     updateMarketPermissions();
     // Request listings
     if (marketWs && marketWs.readyState === 1) {
      marketWs.send(JSON.stringify({ type: 'listing_browse' }));
     }
     // Request stream info
     streamRequestInfo();
     break;
    case 'listing_list':
     marketListings = msg.listings || [];
     renderMarketListings();
     renderMyListings();
     break;
    case 'listing_new':
     if (msg.listing) {
      const existing = marketListings.findIndex(l => l.id === msg.listing.id);
      if (existing >= 0) marketListings[existing] = msg.listing;
      else marketListings.unshift(msg.listing);
      renderMarketListings();
      renderMyListings();
     }
     break;
    case 'listing_updated':
     if (msg.listing) {
      const idx = marketListings.findIndex(l => l.id === msg.listing.id);
      if (idx >= 0) marketListings[idx] = msg.listing;
      renderMarketListings();
      renderMyListings();
     }
     break;
    case 'listing_deleted':
     if (msg.id) {
      marketListings = marketListings.filter(l => l.id !== msg.id);
      renderMarketListings();
      renderMyListings();
     }
     break;
    case 'system':
     // Ignore system messages in market context
     break;
   }
  }

  function updateMarketPermissions() {
   const createBtn = document.getElementById('market-create-btn');
   if (createBtn) createBtn.style.display = (marketCurrentSection === 'marketplace' && canCreateListing()) ? '' : 'none';
  }

  function renderMarketListings() {
   const search = (document.getElementById('market-search').value || '').toLowerCase();
   const catFilter = document.getElementById('market-category-filter').value;
   const condFilter = document.getElementById('market-condition-filter').value;
   const sort = document.getElementById('market-sort').value;

   let filtered = marketListings.filter(l => {
    if (l.status !== 'active') return false;
    if (catFilter && l.category !== catFilter) return false;
    if (condFilter && l.condition !== condFilter) return false;
    if (search && !l.title.toLowerCase().includes(search) && !(l.description||'').toLowerCase().includes(search) && !(l.seller_name||'').toLowerCase().includes(search)) return false;
    return true;
   });

   if (sort === 'oldest') filtered.sort((a, b) => (a.created_at || '').localeCompare(b.created_at || ''));
   else if (sort === 'alpha') filtered.sort((a, b) => a.title.localeCompare(b.title));
   else filtered.sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));

   const grid = document.getElementById('market-listings-grid');
   const empty = document.getElementById('market-listings-empty');
   if (filtered.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
   } else {
    empty.style.display = 'none';
    grid.innerHTML = filtered.map(l => renderListingCard(l, false)).join('');
   }
  }

  function renderListingCard(l, showActions) {
   const catColor = CATEGORY_COLORS[l.category] || '#888';
   const isMine = l.seller_key === marketMyKey;
   const isAdmin = marketMyRole === 'admin' || marketMyRole === 'mod';
   const actions = (isMine || showActions) ? `
    <div style="display:flex;gap:0.3rem;margin-top:0.5rem;">
     ${isMine?`<button onclick="editListing('${l.id}')" style="flex:1;padding:0.25rem;background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--text-muted);cursor:pointer;font-size:0.7rem;">✏️ Edit</button>`:''}
     ${isMine?`<button onclick="markListingSold('${l.id}')" style="flex:1;padding:0.25rem;background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--success);cursor:pointer;font-size:0.7rem;">✅ Sold</button>`:''}
     ${(isMine||isAdmin)?`<button onclick="deleteListing('${l.id}')" style="flex:1;padding:0.25rem;background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--error);cursor:pointer;font-size:0.7rem;">🗑️ Delete</button>`:''}
    </div>` : '';
   const statusBadge = l.status === 'sold' ? '<span style="background:#4a8;color:#fff;font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:4px;margin-left:0.3rem;">SOLD</span>' :
             l.status === 'withdrawn' ? '<span style="background:#888;color:#fff;font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:4px;margin-left:0.3rem;">WITHDRAWN</span>' : '';
   return `
    <div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;overflow:hidden;cursor:pointer;transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'" onclick="showListingDetail('${l.id}')">
     <div style="height:120px;background:linear-gradient(135deg,rgba(255,255,255,0.02),rgba(255,255,255,0.06));display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:2rem;">${l.category === '3D Models' ? '🧊' : '📦'}</div>
     <div style="padding:0.8rem;">
      <div style="display:flex;justify-content:space-between;align-items:start;margin-bottom:0.3rem;">
       <span style="font-weight:600;font-size:0.85rem;color:var(--text);flex:1;">${escHtml(l.title)}${statusBadge}</span>
      </div>
      <div style="font-size:0.95rem;font-weight:700;color:var(--accent);margin-bottom:0.3rem;">${escHtml(l.price || 'Contact for price')}</div>
      <div style="display:flex;gap:0.3rem;flex-wrap:wrap;margin-bottom:0.3rem;">
       <span style="background:${catColor}22;color:${catColor};font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:4px;">${escHtml(l.category)}</span>
       ${l.condition && l.condition !== 'N/A' ? `<span style="background:rgba(255,255,255,0.05);color:var(--text-muted);font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:4px;">${escHtml(l.condition)}</span>` : ''}
      </div>
      <div style="font-size:0.72rem;color:var(--text-muted);">by ${escHtml(l.seller_name || 'Anonymous')}</div>
      ${l.location ? `<div style="font-size:0.68rem;color:var(--text-muted);">📍 ${escHtml(l.location)}</div>` : ''}
      ${actions}
     </div>
    </div>`;
  }

  function showListingDetail(id) {
   const l = marketListings.find(x => x.id === id);
   if (!l) return;
   const isMine = l.seller_key === marketMyKey;
   const isAdmin = marketMyRole === 'admin' || marketMyRole === 'mod';
   const catColor = CATEGORY_COLORS[l.category] || '#888';
   const contactBtn = !isMine ? `<button onclick="contactSeller('${escHtml(l.seller_key)}','${escHtml(l.seller_name||'Seller')}');closeListingDetail()" style="width:100%;padding:0.6rem;background:var(--accent);color:#fff;border:none;border-radius:8px;cursor:pointer;font-size:0.9rem;font-weight:600;margin-top:1rem;">💬 Contact Seller</button>` : '';
   document.getElementById('listing-detail-content').innerHTML = `
    <div style="display:flex;justify-content:space-between;align-items:start;">
     <h2 style="color:var(--text);margin:0;font-size:1.1rem;">${escHtml(l.title)}</h2>
     <button onclick="closeListingDetail()" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:1.2rem;">Close</button>
    </div>
    <div style="font-size:1.3rem;font-weight:700;color:var(--accent);margin:0.8rem 0;">${escHtml(l.price || 'Contact for price')}</div>
    <div style="display:flex;gap:0.4rem;flex-wrap:wrap;margin-bottom:0.8rem;">
     <span style="background:${catColor}22;color:${catColor};font-size:0.7rem;padding:0.15rem 0.5rem;border-radius:4px;">${escHtml(l.category)}</span>
     ${l.condition ? `<span style="background:rgba(255,255,255,0.05);color:var(--text-muted);font-size:0.7rem;padding:0.15rem 0.5rem;border-radius:4px;">${escHtml(l.condition)}</span>` : ''}
     ${l.status !== 'active' ? `<span style="background:${l.status==='sold'?'#4a8':'#888'};color:#fff;font-size:0.7rem;padding:0.15rem 0.5rem;border-radius:4px;">${l.status.toUpperCase()}</span>` : ''}
    </div>
    ${l.description ? `<div style="font-size:0.85rem;color:var(--text);line-height:1.5;margin-bottom:0.8rem;white-space:pre-wrap;">${escHtml(l.description)}</div>` : ''}
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.4rem;font-size:0.8rem;color:var(--text-muted);">
     <div>👤 <strong>${escHtml(l.seller_name || 'Anonymous')}</strong></div>
     ${l.location ? `<div>📍 ${escHtml(l.location)}</div>` : '<div></div>'}
     ${l.payment_methods ? `<div>💳 ${escHtml(l.payment_methods)}</div>` : '<div></div>'}
     <div>📅 ${l.created_at ? new Date(l.created_at).toLocaleDateString() : 'Unknown'}</div>
    </div>
    ${l.category === '3D Models' ? `
    <div style="background:var(--bg-panel);border:1px solid var(--border);border-radius:8px;padding:0.8rem;margin-top:0.8rem;">
     <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.5rem;">
      <span style="font-size:1.5rem;">🧊</span>
      <span style="font-weight:600;color:var(--text);">3D Model</span>
     </div>
     ${(() => {
      const descLower = (l.description||'').toLowerCase();
      const formats = ['blend','stl','obj','gltf','glb'].filter(f => descLower.includes(f));
      const urlMatch = (l.description||'').match(/\/uploads\/[^\s]+/);
      return `
       ${formats.length ? `<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:0.3rem;">Format: ${formats.map(f=>f.toUpperCase()).join(', ')}</div>` : ''}
       ${urlMatch ? `<a href="${escHtml(urlMatch[0])}" download style="display:inline-block;padding:0.4rem 1rem;background:var(--accent);color:#fff;border-radius:6px;text-decoration:none;font-size:0.8rem;font-weight:600;margin-top:0.3rem;">⬇️ Download Model</a>` : ''}
      `;
     })()}
    </div>` : ''}
    ${contactBtn}
    ${isMine || isAdmin ? `
    <div style="display:flex;gap:0.5rem;margin-top:0.8rem;">
     ${isMine?`<button onclick="editListing('${l.id}');closeListingDetail()" class="btn" style="flex:1;min-width:auto;min-height:32px;padding:0.25rem;font-size:0.8rem;">✏️ Edit</button>`:''}
     ${isMine?`<button onclick="markListingSold('${l.id}');closeListingDetail()" class="btn" style="flex:1;min-width:auto;min-height:32px;padding:0.25rem;font-size:0.8rem;color:var(--success);">✅ Mark Sold</button>`:''}
     <button onclick="deleteListing('${l.id}');closeListingDetail()" class="btn" style="flex:1;min-width:auto;min-height:32px;padding:0.25rem;font-size:0.8rem;color:var(--error);">🗑️ Delete</button>
    </div>` : ''}
   `;
   document.getElementById('listing-detail-modal').style.display = '';
  }

  function closeListingDetail() {
   document.getElementById('listing-detail-modal').style.display = 'none';
  }

  function contactSeller(sellerKey, sellerName) {
   // Navigate to chat and open DM — use chat's existing DM system
   // We'll switch to chat tab and trigger a DM open via stored intent
   localStorage.setItem('dm_intent', JSON.stringify({ key: sellerKey, name: sellerName }));
   window.location.href = '/chat';
  }

  function openListingModal(prefill) {
   if (!canCreateListing()) {
    alert('You must be verified to create listings. Ask an admin to verify you.');
    return;
   }
   document.getElementById('listing-modal-title').textContent = prefill && prefill.editId ? 'Edit Listing' : 'Create Listing';
   document.getElementById('listing-edit-id').value = (prefill && prefill.editId) || '';
   document.getElementById('listing-title').value = (prefill && prefill.title) || '';
   document.getElementById('listing-description').value = (prefill && prefill.description) || '';
   document.getElementById('listing-category').value = (prefill && prefill.category) || 'Other';
   document.getElementById('listing-condition').value = (prefill && prefill.condition) || 'Good';
   document.getElementById('listing-price').value = (prefill && prefill.price) || '';
   document.getElementById('listing-payment').value = (prefill && prefill.payment_methods) || '';
   document.getElementById('listing-location').value = (prefill && prefill.location) || '';
   document.getElementById('listing-modal').style.display = '';
   updateListingSubcategory();
  }

  function updateListingSubcategory() {
   const cat = document.getElementById('listing-category').value;
   document.getElementById('listing-3d-subcategory-wrap').style.display = cat === '3D Models' ? '' : 'none';
  }
  // Hook category change
  document.getElementById('listing-category').addEventListener('change', updateListingSubcategory);

  function closeListingModal() {
   document.getElementById('listing-modal').style.display = 'none';
  }

  function submitListing() {
   const title = document.getElementById('listing-title').value.trim();
   if (!title) { document.getElementById('listing-title').style.borderColor = '#e55'; return; }
   const editId = document.getElementById('listing-edit-id').value;
   const data = {
    title,
    description: document.getElementById('listing-description').value.trim(),
    category: document.getElementById('listing-category').value,
    condition: document.getElementById('listing-condition').value,
    price: document.getElementById('listing-price').value.trim(),
    payment_methods: document.getElementById('listing-payment').value.trim(),
    location: document.getElementById('listing-location').value.trim(),
   };
   if (editId) {
    data.id = editId;
    if (marketWs && marketWs.readyState === 1) {
     marketWs.send(JSON.stringify({ type: 'listing_update', ...data }));
    }
   } else {
    data.id = Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
    if (marketWs && marketWs.readyState === 1) {
     marketWs.send(JSON.stringify({ type: 'listing_create', ...data }));
    }
   }
   closeListingModal();
  }

  function editListing(id) {
   const l = marketListings.find(x => x.id === id);
   if (!l) return;
   openListingModal({
    editId: l.id,
    title: l.title,
    description: l.description,
    category: l.category,
    condition: l.condition,
    price: l.price,
    payment_methods: l.payment_methods,
    location: l.location,
   });
  }

  function markListingSold(id) {
   if (!confirm('Mark this listing as sold?')) return;
   if (marketWs && marketWs.readyState === 1) {
    const l = marketListings.find(x => x.id === id);
    if (l) marketWs.send(JSON.stringify({ type: 'listing_update', id, title: l.title, description: l.description, category: l.category, condition: l.condition, price: l.price, payment_methods: l.payment_methods, location: l.location, status: 'sold' }));
   }
  }

  function deleteListing(id) {
   if (!confirm('Delete this listing?')) return;
   if (marketWs && marketWs.readyState === 1) {
    marketWs.send(JSON.stringify({ type: 'listing_delete', id }));
   }
  }

  function renderMyListings() {
   const mine = marketListings.filter(l => l.seller_key === marketMyKey);
   const grid = document.getElementById('my-listings-grid');
   const empty = document.getElementById('my-listings-empty');
   if (mine.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
   } else {
    empty.style.display = 'none';
    grid.innerHTML = mine.map(l => renderListingCard(l, true)).join('');
   }
  }

  // Store Directory
  function renderStoreDirectory() {
   const filter = document.getElementById('store-category-filter').value;
   const filtered = filter ? STORE_DIRECTORY.filter(s => s.category === filter) : STORE_DIRECTORY;
   document.getElementById('store-directory-grid').innerHTML = filtered.map(s => `
    <div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;padding:1rem;transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'">
     <div style="font-size:1.5rem;margin-bottom:0.4rem;">${s.icon}</div>
     <div style="font-weight:600;font-size:0.9rem;color:var(--text);margin-bottom:0.2rem;">${escHtml(s.name)}</div>
     <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.4rem;">${escHtml(s.category)}</div>
     <div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:0.6rem;">${escHtml(s.description)}</div>
     <a href="${s.url}" target="_blank" rel="noopener" style="display:inline-block;padding:0.3rem 0.8rem;background:var(--accent);color:#fff;border-radius:6px;text-decoration:none;font-size:0.75rem;font-weight:600;">Visit Store →</a>
    </div>`).join('');
  }

  // Populate store category filter
  (function() {
   const cats = [...new Set(STORE_DIRECTORY.map(s => s.category))];
   const sel = document.getElementById('store-category-filter');
   cats.forEach(c => { sel.innerHTML += `<option value="${escHtml(c)}">${escHtml(c)}</option>`; });
  })();

  // Inventory → Listing bridge
  function listInventoryForSale(itemId) {
   const item = loadInventory().find(i => i.id === itemId);
   if (!item) return;
   // Map inventory category to marketplace category
   const catMap = {'Vehicle':'Vehicles','Clothing':'Clothing','Electronics':'Electronics','Tools':'Tools','Furniture':'Furniture','Kitchen':'Home','Books/Media':'Books/Media','Gaming':'Gaming','Home':'Home','Other':'Other'};
   const category = catMap[item.category] || 'Other';
   // Switch to market tab
   switchTab('market');
   showMarketSection('marketplace');
   openListingModal({
    title: item.name,
    description: (item.description || '') + (item.notes ? '\n\n' + item.notes : ''),
    category: category,
    condition: item.condition || 'Good',
   });
  }

  // Connect marketplace WS when market tab is shown
  // Connect WS immediately — needed for streaming, skills, board, etc.
  marketConnect();

  // ══════════════════════════════════════
  // UNIVERSAL SEARCH
  // ══════════════════════════════════════
  function universalSearch(query) {
   const results = document.getElementById('universal-search-results');
   if (!query || query.length < 2) { results.innerHTML = ''; return; }
   const q = query.toLowerCase();
   const hits = [];
   // Elements
   ELEMENTS.filter(e => e.name.toLowerCase().includes(q) || e.symbol.toLowerCase().includes(q) || String(e.number).includes(q))
    .forEach(e => hits.push({ label: `${e.symbol} — ${e.name} (#${e.number})`, badge: 'Element', color: '#4caf50', action: () => { showElementDetail(e.symbol); } }));
   // Materials
   MATERIALS.filter(m => m.name.toLowerCase().includes(q) || m.category.toLowerCase().includes(q))
    .forEach(m => hits.push({ label: m.name + ' (' + m.type + ')', badge: 'Material', color: '#42a5f5', action: () => { showMaterialDetail(m.name); } }));
   // Inventory
   loadInventory().filter(i => i.name.toLowerCase().includes(q) || (i.category||'').toLowerCase().includes(q) || (i.tags||[]).some(t => t.toLowerCase().includes(q)))
    .forEach(i => hits.push({ label: i.name + (i.category ? ' · ' + i.category : ''), badge: 'Inventory', color: '#ffa726', action: () => { editInventoryItem(i.id); } }));
   results.innerHTML = hits.length === 0 ? '<div style="padding:0.8rem;text-align:center;font-size:0.82rem;color:var(--text-muted);">No results</div>'
    : hits.slice(0, 20).map((h, i) => `<div class="catalog-result-item" onclick="universalSearchResults[${i}]()">
      <span class="catalog-result-badge" style="background:${h.color}22;color:${h.color};">${h.badge}</span>
      <span style="color:var(--text);">${escHtml(h.label)}</span>
     </div>`).join('');
   window.universalSearchResults = hits.slice(0, 20).map(h => h.action);
  }

  // ── TODO ──
  function loadTodos() {
   try { return JSON.parse(localStorage.getItem('humanity_todos')) || []; } catch { return []; }
  }
  function saveTodos(todos) { localStorage.setItem('humanity_todos', JSON.stringify(todos)); }

  function renderTodos() {
   const todos = loadTodos();
   const list = document.getElementById('todo-list');
   const active = todos.filter(t => !t.completed);
   const done = todos.filter(t => t.completed);
   const sorted = [...active, ...done];
   list.innerHTML = sorted.length === 0 ? '<div style="color:var(--text-muted);font-size:0.8rem;font-style:italic;padding:0.5rem;">No tasks yet</div>' : '';
   sorted.forEach(t => {
    const div = document.createElement('div');
    div.className = 'todo-item' + (t.completed ? ' completed' : '');
    div.innerHTML = `<input type="checkbox" ${t.completed ? 'checked' : ''} onchange="toggleTodo('${t.id}')"><span class="todo-text">${escHtml(t.text)}</span><button class="todo-delete" onclick="deleteTodo('${t.id}')" title="Delete">Close</button>`;
    list.appendChild(div);
   });
  }

  function addTodo() {
   const input = document.getElementById('todo-input');
   const text = input.value.trim();
   if (!text) return;
   const todos = loadTodos();
   todos.push({ id: Date.now().toString(36), text, completed: false, createdAt: new Date().toISOString() });
   saveTodos(todos);
   input.value = '';
   renderTodos();
  }

  function toggleTodo(id) {
   const todos = loadTodos();
   const t = todos.find(x => x.id === id);
   if (t) t.completed = !t.completed;
   saveTodos(todos);
   renderTodos();
  }

  function deleteTodo(id) {
   saveTodos(loadTodos().filter(x => x.id !== id));
   renderTodos();
  }

  // ── NOTES ──
  let currentNoteId = null;

  function loadNotes() {
   try { return JSON.parse(localStorage.getItem('humanity_notes')) || []; } catch { return []; }
  }
  function saveNotes(notes) { localStorage.setItem('humanity_notes', JSON.stringify(notes)); }

  function renderNotes() {
   const notes = loadNotes();
   const list = document.getElementById('notes-list');
   list.innerHTML = notes.length === 0 ? '<div style="color:var(--text-muted);font-size:0.8rem;font-style:italic;padding:0.3rem;">No notes yet</div>' : '';
   notes.forEach(n => {
    const div = document.createElement('div');
    div.className = 'note-item' + (n.id === currentNoteId ? ' active' : '');
    div.innerHTML = `<span onclick="selectNote('${n.id}')">${escHtml(n.title || 'Untitled')}</span><button class="note-delete" onclick="event.stopPropagation();deleteNote('${n.id}')" title="Delete">Close</button>`;
    div.querySelector('span').style.flex = '1';
    div.querySelector('span').style.cursor = 'pointer';
    list.appendChild(div);
   });
  }

  function addNote() {
   const notes = loadNotes();
   const id = Date.now().toString(36);
   notes.unshift({ id, title: '', content: '', updatedAt: new Date().toISOString() });
   saveNotes(notes);
   selectNote(id);
  }

  function selectNote(id) {
   currentNoteId = id;
   const notes = loadNotes();
   const note = notes.find(n => n.id === id);
   const editor = document.getElementById('notes-editor');
   if (note) {
    document.getElementById('note-title').value = note.title;
    document.getElementById('note-content').value = note.content;
    editor.style.display = 'block';
   }
   renderNotes();
  }

  function saveCurrentNote() {
   if (!currentNoteId) return;
   const notes = loadNotes();
   const note = notes.find(n => n.id === currentNoteId);
   if (note) {
    note.title = document.getElementById('note-title').value;
    note.content = document.getElementById('note-content').value;
    note.updatedAt = new Date().toISOString();
    saveNotes(notes);
    renderNotes();
   }
  }

  function deleteNote(id) {
   saveNotes(loadNotes().filter(n => n.id !== id));
   if (currentNoteId === id) {
    currentNoteId = null;
    document.getElementById('notes-editor').style.display = 'none';
   }
   renderNotes();
  }

  // ── GARDEN MANAGER v2 ──
  const OPTIMAL_RANGES = {
   soil: { ph:{min:6.0,max:7.0,unit:'',label:'pH'}, moisture:{min:40,max:60,unit:'%',label:'Soil Moisture'}, temperature:{min:60,max:85,unit:'°F',label:'Air Temperature'} },
   hydroponic: { ph:{min:5.5,max:6.5,unit:'',label:'pH'}, ec:{min:1.0,max:2.5,unit:'mS/cm',label:'EC'}, waterTemp:{min:65,max:75,unit:'°F',label:'Water Temp'}, dissolvedO2:{min:5,max:8,unit:'mg/L',label:'Dissolved O2'} },
   aeroponic: { ph:{min:5.5,max:6.5,unit:'',label:'pH'}, ec:{min:1.0,max:2.0,unit:'mS/cm',label:'EC'}, rootTemp:{min:65,max:72,unit:'°F',label:'Root Zone Temp'} },
   aquaponic: { ph:{min:6.8,max:7.2,unit:'',label:'pH'}, ammonia:{min:0,max:0.5,unit:'ppm',label:'Ammonia'}, nitrite:{min:0,max:0.5,unit:'ppm',label:'Nitrite'}, nitrate:{min:10,max:150,unit:'ppm',label:'Nitrate'}, waterTemp:{min:72,max:82,unit:'°F',label:'Water Temp'}, dissolvedO2:{min:5,max:8,unit:'mg/L',label:'Dissolved O2'} },
   general: { humidity:{min:40,max:70,unit:'%',label:'Humidity'}, co2:{min:400,max:1500,unit:'ppm',label:'CO2'}, lightPar:{min:200,max:800,unit:'µmol/m²/s',label:'PPFD'}, lightDli:{min:12,max:40,unit:'mol/m²/day',label:'DLI'} }
  };

  const ZONE_ICONS = { field:'🌾', raised_bed:'🌿', container:'🪴', hydro_dwc:'💧', hydro_nft:'💧', hydro_drip:'💧', hydro_ebb_flow:'💧', aeroponic:'🌫️', aquaponic:'🐟' };
  const ZONE_LABELS = { field:'Field', raised_bed:'Raised Bed', container:'Container', hydro_dwc:'Hydro DWC', hydro_nft:'Hydro NFT', hydro_drip:'Hydro Drip', hydro_ebb_flow:'Hydro Ebb/Flow', aeroponic:'Aeroponic', aquaponic:'Aquaponic' };
  const STAGE_ICONS = { seed:'🌰', germination:'🌱', seedling:'🌿', vegetative:'🪴', flowering:'🌸', fruiting:'🍎', harvest:'🧺', dormant:'💤' };
  const HEALTH_ICONS = { healthy:'✓', stressed:'⚠', diseased:'🦠', pest:'🐛', nutrient_deficiency:'🟡', overwatered:'💦', underwatered:'🏜️' };

  let gardenData = null;
  let gardenSelectedZone = null;

  function loadGardenV2() {
   if (gardenData) return gardenData;
   try { gardenData = JSON.parse(localStorage.getItem('humanity_garden_v2')) || { zones:[], plants:[], sensors:[], settings:{units:'imperial'} }; }
   catch { gardenData = { zones:[], plants:[], sensors:[], settings:{units:'imperial'} }; }
   return gardenData;
  }
  function saveGardenV2() { localStorage.setItem('humanity_garden_v2', JSON.stringify(gardenData)); }

  function gardenRangeColor(value, range) {
   if (value == null || !range) return '';
   const span = range.max - range.min;
   const margin = span * 0.1;
   if (value >= range.min && value <= range.max) return '#4a8';
   if (value >= range.min - margin && value <= range.max + margin) return '#eb4';
   return '#e55';
  }

  function gardenGetRanges(zone) {
   const t = zone.type;
   if (t.startsWith('hydro')) return {...OPTIMAL_RANGES.general, ...OPTIMAL_RANGES.hydroponic};
   if (t === 'aeroponic') return {...OPTIMAL_RANGES.general, ...OPTIMAL_RANGES.aeroponic};
   if (t === 'aquaponic') return {...OPTIMAL_RANGES.general, ...OPTIMAL_RANGES.aquaponic};
   return {...OPTIMAL_RANGES.general, ...OPTIMAL_RANGES.soil};
  }

  function renderGardenZoneTree() {
   const g = loadGardenV2();
   const tree = document.getElementById('garden-zone-tree');
   const grouped = { outdoor:[], indoor:[], greenhouse:[] };
   g.zones.forEach(z => { (grouped[z.location] || grouped.outdoor).push(z); });
   const locLabels = { outdoor:'📁 Outdoor', indoor:'📁 Indoor', greenhouse:'📁 Greenhouse' };
   let html = '';
   for (const [loc, zones] of Object.entries(grouped)) {
    if (zones.length === 0) continue;
    html += `<div style="margin-bottom:0.4rem;">
     <div style="font-weight:600;font-size:0.72rem;color:var(--text-muted);padding:0.2rem 0.3rem;cursor:pointer;user-select:none;" onclick="this.nextElementSibling.style.display=this.nextElementSibling.style.display==='none'?'block':'none'">${locLabels[loc]}</div>
     <div>`;
    zones.forEach(z => {
     const sel = gardenSelectedZone === z.id ? 'background:rgba(255,136,17,0.15);color:var(--accent);font-weight:600;' : '';
     const plantCount = g.plants.filter(p => p.zoneId === z.id).length;
     html += `<div onclick="gardenSelectZone('${z.id}')" style="padding:0.25rem 0.4rem 0.25rem 1rem;cursor:pointer;border-radius:4px;font-size:0.78rem;display:flex;align-items:center;gap:0.3rem;${sel}" title="${ZONE_LABELS[z.type]}">
      <span>${ZONE_ICONS[z.type]||'🌱'}</span>
      <span style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escHtml(z.name)}</span>
      <span style="font-size:0.65rem;color:var(--text-muted);">${plantCount}</span>
     </div>`;
    });
    html += '</div></div>';
   }
   if (g.zones.length === 0) html = '<div style="text-align:center;color:var(--text-muted);font-size:0.75rem;padding:1rem;">No zones</div>';
   tree.innerHTML = html;
   document.getElementById('garden-empty').style.display = g.zones.length === 0 ? '' : 'none';
   document.getElementById('garden-zone-detail').style.display = g.zones.length > 0 && gardenSelectedZone ? '' : 'none';
  }

  function gardenSelectZone(zoneId) {
   gardenSelectedZone = zoneId;
   renderGardenZoneTree();
   renderGardenZoneDetail();
  }

  function renderGardenZoneDetail() {
   const g = loadGardenV2();
   const zone = g.zones.find(z => z.id === gardenSelectedZone);
   const det = document.getElementById('garden-zone-detail');
   if (!zone) { det.style.display = 'none'; return; }
   det.style.display = '';
   const plants = g.plants.filter(p => p.zoneId === zone.id);
   const sensor = g.sensors.find(s => s.zoneId === zone.id);
   const lastReading = sensor && sensor.readings.length > 0 ? sensor.readings[sensor.readings.length - 1].metrics : {};
   const ranges = gardenGetRanges(zone);
   const dim = zone.dimensions ? `${zone.dimensions.length}×${zone.dimensions.width} ${zone.dimensions.unit}` : '—';

   // Dashboard cards
   const metricKeys = [
    ['ph','pH',lastReading.ph],['ec','EC',lastReading.ec],['airTemp','Temp',lastReading.airTemp],
    ['humidity','RH',lastReading.humidity],['soilMoisture','Moisture',lastReading.soilMoisture],
    ['lightPar','PPFD',lastReading.lightPar],['waterTemp','Water',lastReading.waterTemp],
    ['dissolvedO2','DO',lastReading.dissolvedO2],['co2','CO2',lastReading.co2]
   ].filter(m => m[2] != null);

   let dashHtml = '';
   metricKeys.forEach(([key,label,val]) => {
    const r = ranges[key];
    const col = gardenRangeColor(val, r);
    const unit = r ? r.unit : '';
    dashHtml += `<div style="background:var(--bg-input);border-radius:6px;padding:0.4rem 0.6rem;text-align:center;border-left:3px solid ${col||'var(--border)'};">
     <div style="font-size:0.65rem;color:var(--text-muted);">${label}</div>
     <div style="font-size:1rem;font-weight:700;color:${col||'var(--text)'}">${val}${unit?' '+unit:''}</div>
    </div>`;
   });

   // Plant list
   let plantHtml = '';
   plants.forEach(p => {
    const hIcon = HEALTH_ICONS[p.health] || '✓';
    const hCol = p.health === 'healthy' ? '#4a8' : (p.health === 'stressed' || p.health === 'nutrient_deficiency' ? '#eb4' : '#e55');
    plantHtml += `<div onclick="gardenShowPlant('${p.id}')" style="display:flex;align-items:center;gap:0.5rem;padding:0.35rem 0.5rem;cursor:pointer;border-radius:4px;font-size:0.82rem;" onmouseover="this.style.background='var(--bg-hover)'" onmouseout="this.style.background=''">
     <span>${STAGE_ICONS[p.stage]||'🌱'}</span>
     <span style="flex:1;">${escHtml(p.species)}${p.variety?' — '+escHtml(p.variety):''}</span>
     <span style="font-size:0.7rem;color:var(--text-muted);">[${p.stage}]</span>
     <span style="color:${hCol};font-size:0.8rem;" title="${p.health}">${hIcon}</span>
    </div>`;
   });

   // Readings chart (simple canvas line for pH over last 7 readings)
   const chartId = 'garden-chart-' + zone.id;

   det.innerHTML = `
    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:0.8rem;">
     <div>
      <div style="font-size:1.1rem;font-weight:700;color:var(--text);">${ZONE_ICONS[zone.type]||'🌱'} ${escHtml(zone.name)}</div>
      <div style="font-size:0.78rem;color:var(--text-muted);">Type: ${ZONE_LABELS[zone.type]} (${zone.medium || 'soil'}) · Size: ${dim} · Plants: ${plants.length} · Light: ${zone.lightSource||'natural'} ${zone.lightSchedule||''}</div>
     </div>
     <div style="display:flex;gap:0.3rem;">
      <button onclick="gardenEditZone('${zone.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text-muted);padding:0.2rem 0.5rem;border-radius:4px;font-size:0.7rem;cursor:pointer;">✏️ Edit</button>
      <button onclick="gardenDeleteZone('${zone.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--error);padding:0.2rem 0.5rem;border-radius:4px;font-size:0.7rem;cursor:pointer;">🗑️</button>
     </div>
    </div>
    ${metricKeys.length > 0 ? `<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(100px,1fr));gap:0.4rem;margin-bottom:1rem;">${dashHtml}</div>` : ''}
    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:0.4rem;">
     <div style="font-weight:600;font-size:0.85rem;color:var(--text);">Plants</div>
     <button onclick="gardenAddPlant('${zone.id}')" style="background:var(--accent);color:#fff;border:none;padding:0.15rem 0.5rem;border-radius:4px;font-size:0.7rem;cursor:pointer;font-weight:600;">+ Add Plant</button>
    </div>
    ${plants.length > 0 ? plantHtml : '<div style="color:var(--text-muted);font-size:0.8rem;font-style:italic;padding:0.5rem;">No plants yet</div>'}
    <div style="display:flex;gap:0.3rem;margin-top:0.8rem;">
     <button onclick="gardenLogReading('${zone.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">Quality Log Reading</button>
    </div>
    ${sensor && sensor.readings.length > 1 ? `<div style="margin-top:1rem;"><div style="font-weight:600;font-size:0.85rem;color:var(--text);margin-bottom:0.4rem;">Readings (last 7)</div><canvas id="${chartId}" width="500" height="140" style="width:100%;border-radius:6px;background:var(--bg-input);"></canvas></div>` : ''}
    ${sensor && sensor.readings.length > 0 ? gardenReadingsTable(sensor, ranges) : ''}
    ${zone.notes ? `<div style="margin-top:0.8rem;font-size:0.78rem;color:var(--text-muted);"><strong>Notes:</strong> ${escHtml(zone.notes)}</div>` : ''}
   `;

   // Draw chart
   if (sensor && sensor.readings.length > 1) {
    setTimeout(() => gardenDrawChart(chartId, sensor.readings.slice(-7)), 50);
   }
  }

  function gardenReadingsTable(sensor, ranges) {
   const readings = sensor.readings.slice(-10).reverse();
   let html = '<div style="margin-top:0.8rem;overflow-x:auto;"><table style="width:100%;font-size:0.72rem;border-collapse:collapse;">';
   html += '<tr style="border-bottom:1px solid var(--border);color:var(--text-muted);"><th style="padding:0.2rem 0.4rem;text-align:left;">Date</th><th>pH</th><th>EC</th><th>Temp</th><th>RH</th><th>Moisture</th><th>PPFD</th></tr>';
   readings.forEach(r => {
    const m = r.metrics;
    const d = new Date(r.date).toLocaleDateString();
    const c = (k,v) => { const col = gardenRangeColor(v, ranges[k]); return col ? `color:${col}` : ''; };
    html += `<tr style="border-bottom:1px solid rgba(255,255,255,0.03);">
     <td style="padding:0.2rem 0.4rem;">${d}</td>
     <td style="text-align:center;${c('ph',m.ph)}">${m.ph??'—'}</td>
     <td style="text-align:center;${c('ec',m.ec)}">${m.ec??'—'}</td>
     <td style="text-align:center;${c('airTemp',m.airTemp)}">${m.airTemp??'—'}</td>
     <td style="text-align:center;${c('humidity',m.humidity)}">${m.humidity??'—'}</td>
     <td style="text-align:center;${c('soilMoisture',m.soilMoisture)}">${m.soilMoisture??'—'}</td>
     <td style="text-align:center;${c('lightPar',m.lightPar)}">${m.lightPar??'—'}</td>
    </tr>`;
   });
   html += '</table></div>';
   return html;
  }

  function gardenDrawChart(canvasId, readings) {
   const canvas = document.getElementById(canvasId);
   if (!canvas) return;
   const ctx = canvas.getContext('2d');
   const w = canvas.width, h = canvas.height;
   ctx.clearRect(0,0,w,h);
   const phVals = readings.map(r => r.metrics.ph).filter(v => v != null);
   if (phVals.length < 2) return;
   const min = Math.min(...phVals) - 0.5, max = Math.max(...phVals) + 0.5;
   const pad = {t:20,b:20,l:30,r:10};
   ctx.strokeStyle = '#4a8'; ctx.lineWidth = 2; ctx.beginPath();
   phVals.forEach((v,i) => {
    const x = pad.l + (i / (phVals.length-1)) * (w-pad.l-pad.r);
    const y = pad.t + (1 - (v-min)/(max-min)) * (h-pad.t-pad.b);
    i===0 ? ctx.moveTo(x,y) : ctx.lineTo(x,y);
   });
   ctx.stroke();
   // dots
   phVals.forEach((v,i) => {
    const x = pad.l + (i / (phVals.length-1)) * (w-pad.l-pad.r);
    const y = pad.t + (1 - (v-min)/(max-min)) * (h-pad.t-pad.b);
    ctx.fillStyle='#4a8'; ctx.beginPath(); ctx.arc(x,y,3,0,Math.PI*2); ctx.fill();
   });
   // labels
   ctx.fillStyle='#888'; ctx.font='10px sans-serif'; ctx.textAlign='right';
   ctx.fillText(max.toFixed(1), pad.l-4, pad.t+4);
   ctx.fillText(min.toFixed(1), pad.l-4, h-pad.b+4);
   ctx.fillText('pH', pad.l-4, 12);
  }

  function gardenAddZone() {
   document.getElementById('gz-edit-id').value = '';
   document.getElementById('gz-name').value = '';
   document.getElementById('gz-type').value = 'raised_bed';
   document.getElementById('gz-location').value = 'outdoor';
   document.getElementById('gz-length').value = '';
   document.getElementById('gz-width').value = '';
   document.getElementById('gz-unit').value = 'ft';
   document.getElementById('gz-medium').value = 'soil';
   document.getElementById('gz-light').value = 'natural';
   document.getElementById('gz-schedule').value = '';
   document.getElementById('gz-notes').value = '';
   document.getElementById('garden-zone-modal-title').textContent = 'New Zone';
   document.getElementById('garden-zone-modal').classList.add('open');
  }

  function gardenEditZone(id) {
   const g = loadGardenV2();
   const z = g.zones.find(x => x.id === id);
   if (!z) return;
   document.getElementById('gz-edit-id').value = z.id;
   document.getElementById('gz-name').value = z.name;
   document.getElementById('gz-type').value = z.type;
   document.getElementById('gz-location').value = z.location;
   document.getElementById('gz-length').value = z.dimensions?.length || '';
   document.getElementById('gz-width').value = z.dimensions?.width || '';
   document.getElementById('gz-unit').value = z.dimensions?.unit || 'ft';
   document.getElementById('gz-medium').value = z.medium || 'soil';
   document.getElementById('gz-light').value = z.lightSource || 'natural';
   document.getElementById('gz-schedule').value = z.lightSchedule || '';
   document.getElementById('gz-notes').value = z.notes || '';
   document.getElementById('garden-zone-modal-title').textContent = 'Edit Zone';
   document.getElementById('garden-zone-modal').classList.add('open');
  }

  function gardenCloseZoneModal() { document.getElementById('garden-zone-modal').classList.remove('open'); }

  function gardenSaveZone() {
   const g = loadGardenV2();
   const editId = document.getElementById('gz-edit-id').value;
   const name = document.getElementById('gz-name').value.trim();
   if (!name) { alert('Name is required'); return; }
   const zoneData = {
    name, type: document.getElementById('gz-type').value,
    location: document.getElementById('gz-location').value,
    dimensions: { length: parseFloat(document.getElementById('gz-length').value)||0, width: parseFloat(document.getElementById('gz-width').value)||0, unit: document.getElementById('gz-unit').value },
    medium: document.getElementById('gz-medium').value,
    lightSource: document.getElementById('gz-light').value,
    lightSchedule: document.getElementById('gz-schedule').value,
    notes: document.getElementById('gz-notes').value
   };
   if (editId) {
    const idx = g.zones.findIndex(z => z.id === editId);
    if (idx >= 0) Object.assign(g.zones[idx], zoneData);
   } else {
    zoneData.id = 'zone-' + Date.now();
    zoneData.createdAt = Date.now();
    g.zones.push(zoneData);
    gardenSelectedZone = zoneData.id;
   }
   gardenData = g; saveGardenV2();
   gardenCloseZoneModal();
   renderGardenZoneTree();
   renderGardenZoneDetail();
  }

  function gardenDeleteZone(id) {
   if (!confirm('Delete this zone and all its plants?')) return;
   const g = loadGardenV2();
   g.zones = g.zones.filter(z => z.id !== id);
   g.plants = g.plants.filter(p => p.zoneId !== id);
   g.sensors = g.sensors.filter(s => s.zoneId !== id);
   if (gardenSelectedZone === id) gardenSelectedZone = g.zones.length > 0 ? g.zones[0].id : null;
   gardenData = g; saveGardenV2();
   renderGardenZoneTree();
   renderGardenZoneDetail();
  }

  function gardenAddPlant(zoneId) {
   document.getElementById('gp-edit-id').value = '';
   document.getElementById('gp-zone-id').value = zoneId;
   document.getElementById('gp-species').value = '';
   document.getElementById('gp-variety').value = '';
   document.getElementById('gp-stage').value = 'vegetative';
   document.getElementById('gp-health').value = 'healthy';
   document.getElementById('gp-planted').value = new Date().toISOString().split('T')[0];
   document.getElementById('gp-harvest').value = '';
   document.getElementById('gp-row').value = '';
   document.getElementById('gp-col').value = '';
   document.getElementById('gp-slot').value = '';
   document.getElementById('gp-notes').value = '';
   document.getElementById('garden-plant-modal-title2').textContent = 'Add Plant';
   document.getElementById('garden-add-plant-modal').classList.add('open');
  }

  function gardenCloseAddPlantModal() { document.getElementById('garden-add-plant-modal').classList.remove('open'); }

  function gardenSavePlant() {
   const g = loadGardenV2();
   const editId = document.getElementById('gp-edit-id').value;
   const species = document.getElementById('gp-species').value.trim();
   if (!species) { alert('Species is required'); return; }
   const zoneId = document.getElementById('gp-zone-id').value;
   const plantData = {
    zoneId, species, variety: document.getElementById('gp-variety').value.trim(),
    stage: document.getElementById('gp-stage').value, health: document.getElementById('gp-health').value,
    plantedDate: document.getElementById('gp-planted').value ? new Date(document.getElementById('gp-planted').value).getTime() : null,
    expectedHarvest: document.getElementById('gp-harvest').value ? new Date(document.getElementById('gp-harvest').value).getTime() : null,
    position: { row: parseInt(document.getElementById('gp-row').value)||null, col: parseInt(document.getElementById('gp-col').value)||null },
    slot: parseInt(document.getElementById('gp-slot').value)||null,
    notes: document.getElementById('gp-notes').value, yields:[], waterings:[], prunings:[], photos:[]
   };
   if (editId) {
    const idx = g.plants.findIndex(p => p.id === editId);
    if (idx >= 0) { const old = g.plants[idx]; Object.assign(old, plantData); old.yields = old.yields||[]; old.waterings = old.waterings||[]; }
   } else {
    plantData.id = 'plant-' + Date.now();
    g.plants.push(plantData);
   }
   gardenData = g; saveGardenV2();
   gardenCloseAddPlantModal();
   renderGardenZoneDetail();
  }

  function gardenShowPlant(plantId) {
   const g = loadGardenV2();
   const p = g.plants.find(x => x.id === plantId);
   if (!p) return;
   const zone = g.zones.find(z => z.id === p.zoneId);
   const stages = ['seed','germination','seedling','vegetative','flowering','fruiting','harvest','dormant'];
   const stageIdx = stages.indexOf(p.stage);
   let timelineHtml = '<div style="display:flex;gap:0.15rem;margin:0.6rem 0;">';
   stages.forEach((s,i) => {
    const active = i <= stageIdx;
    timelineHtml += `<div style="flex:1;height:6px;border-radius:3px;background:${active?'#4a8':'var(--bg-input)'}" title="${s}"></div>`;
   });
   timelineHtml += '</div>';

   let yieldHtml = '';
   if (p.yields && p.yields.length > 0) {
    yieldHtml = '<div style="margin-top:0.6rem;"><strong style="font-size:0.78rem;">Yield History</strong>';
    p.yields.forEach(y => { yieldHtml += `<div style="font-size:0.75rem;color:var(--text-muted);">${new Date(y.date).toLocaleDateString()} — ${y.amount} ${y.unit}</div>`; });
    yieldHtml += '</div>';
   }

   let waterHtml = '';
   if (p.waterings && p.waterings.length > 0) {
    waterHtml = '<div style="margin-top:0.6rem;"><strong style="font-size:0.78rem;">Watering Log</strong>';
    p.waterings.slice(-5).forEach(w => { waterHtml += `<div style="font-size:0.75rem;color:var(--text-muted);">${new Date(w.date).toLocaleDateString()} — ${w.amount}${w.nutrientMix?' ('+w.nutrientMix+')':''}</div>`; });
    waterHtml += '</div>';
   }

   document.getElementById('garden-plant-modal-content').innerHTML = `
    <div style="display:flex;align-items:center;gap:0.6rem;margin-bottom:0.6rem;">
     <span style="font-size:1.5rem;">${STAGE_ICONS[p.stage]||'🌱'}</span>
     <div>
      <div style="font-size:1.1rem;font-weight:700;color:var(--text);">${escHtml(p.species)}${p.variety?' — '+escHtml(p.variety):''}</div>
      <div style="font-size:0.78rem;color:var(--text-muted);">Zone: ${zone?escHtml(zone.name):'—'} · Stage: ${p.stage} · Health: ${p.health}</div>
     </div>
    </div>
    <div style="font-size:0.78rem;color:var(--text-muted);">Growth Timeline</div>
    ${timelineHtml}
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;font-size:0.8rem;">
     <div><strong>Planted:</strong> ${p.plantedDate?new Date(p.plantedDate).toLocaleDateString():'—'}</div>
     <div><strong>Expected Harvest:</strong> ${p.expectedHarvest?new Date(p.expectedHarvest).toLocaleDateString():'—'}</div>
     ${p.position?.row?`<div><strong>Position:</strong> Row ${p.position.row}, Col ${p.position.col||'—'}</div>`:''}
     ${p.slot?`<div><strong>Slot:</strong> ${p.slot}</div>`:''}
    </div>
    ${yieldHtml}
    ${waterHtml}
    ${p.notes?`<div style="margin-top:0.6rem;font-size:0.8rem;"><strong>Notes:</strong> ${escHtml(p.notes)}</div>`:''}
    <div style="display:flex;gap:0.3rem;margin-top:1rem;">
     <button onclick="gardenEditPlant('${p.id}')" style="background:var(--accent);color:#fff;border:none;padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">✏️ Edit</button>
     <button onclick="gardenLogWatering('${p.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">💧 Water</button>
     <button onclick="gardenLogYield('${p.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">🧺 Yield</button>
     <button onclick="gardenDeletePlant('${p.id}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--error);padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">🗑️</button>
    </div>
   `;
   document.getElementById('garden-plant-modal').classList.add('open');
  }

  function gardenClosePlantModal() { document.getElementById('garden-plant-modal').classList.remove('open'); }

  function gardenEditPlant(id) {
   gardenClosePlantModal();
   const g = loadGardenV2();
   const p = g.plants.find(x => x.id === id);
   if (!p) return;
   document.getElementById('gp-edit-id').value = p.id;
   document.getElementById('gp-zone-id').value = p.zoneId;
   document.getElementById('gp-species').value = p.species;
   document.getElementById('gp-variety').value = p.variety||'';
   document.getElementById('gp-stage').value = p.stage;
   document.getElementById('gp-health').value = p.health;
   document.getElementById('gp-planted').value = p.plantedDate ? new Date(p.plantedDate).toISOString().split('T')[0] : '';
   document.getElementById('gp-harvest').value = p.expectedHarvest ? new Date(p.expectedHarvest).toISOString().split('T')[0] : '';
   document.getElementById('gp-row').value = p.position?.row||'';
   document.getElementById('gp-col').value = p.position?.col||'';
   document.getElementById('gp-slot').value = p.slot||'';
   document.getElementById('gp-notes').value = p.notes||'';
   document.getElementById('garden-plant-modal-title2').textContent = 'Edit Plant';
   document.getElementById('garden-add-plant-modal').classList.add('open');
  }

  function gardenDeletePlant(id) {
   if (!confirm('Delete this plant?')) return;
   const g = loadGardenV2();
   g.plants = g.plants.filter(p => p.id !== id);
   gardenData = g; saveGardenV2();
   gardenClosePlantModal();
   renderGardenZoneDetail();
  }

  function gardenLogWatering(plantId) {
   const amount = prompt('Water amount (e.g. "1 gal")');
   if (!amount) return;
   const mix = prompt('Nutrient mix (optional)', '');
   const g = loadGardenV2();
   const p = g.plants.find(x => x.id === plantId);
   if (!p) return;
   if (!p.waterings) p.waterings = [];
   p.waterings.push({ date: Date.now(), amount, nutrientMix: mix||'' });
   gardenData = g; saveGardenV2();
   gardenShowPlant(plantId);
  }

  function gardenLogYield(plantId) {
   const amount = prompt('Yield amount (number)');
   if (!amount) return;
   const unit = prompt('Unit (e.g. lb, oz, kg)', 'lb');
   const g = loadGardenV2();
   const p = g.plants.find(x => x.id === plantId);
   if (!p) return;
   if (!p.yields) p.yields = [];
   p.yields.push({ date: Date.now(), amount: parseFloat(amount)||0, unit: unit||'lb' });
   gardenData = g; saveGardenV2();
   gardenShowPlant(plantId);
  }

  function gardenLogReading(zoneId) {
   document.getElementById('gr-zone-id').value = zoneId;
   ['ph','ec','tds','watertemp','airtemp','humidity','moisture','par','co2','do','waterlevel','dli'].forEach(k => {
    document.getElementById('gr-'+k).value = '';
   });
   document.getElementById('garden-reading-modal').classList.add('open');
  }

  function gardenSaveReading() {
   const g = loadGardenV2();
   const zoneId = document.getElementById('gr-zone-id').value;
   let sensor = g.sensors.find(s => s.zoneId === zoneId);
   if (!sensor) { sensor = { id:'sensor-'+Date.now(), zoneId, type:'manual', readings:[] }; g.sensors.push(sensor); }
   const metrics = {};
   const map = {ph:'ph',ec:'ec',tds:'tds',watertemp:'waterTemp',airtemp:'airTemp',humidity:'humidity',moisture:'soilMoisture',par:'lightPar',co2:'co2',do:'dissolvedO2',waterlevel:'waterLevel',dli:'lightDli'};
   for (const [k,v] of Object.entries(map)) {
    const val = parseFloat(document.getElementById('gr-'+k).value);
    if (!isNaN(val)) metrics[v] = val;
   }
   if (Object.keys(metrics).length === 0) { alert('Enter at least one reading'); return; }
   sensor.readings.push({ date: Date.now(), metrics });
   gardenData = g; saveGardenV2();
   document.getElementById('garden-reading-modal').classList.remove('open');
   renderGardenZoneDetail();
  }

  function renderGarden() {
   loadGardenV2();
   if (gardenData.zones.length > 0 && !gardenSelectedZone) gardenSelectedZone = gardenData.zones[0].id;
   renderGardenZoneTree();
   if (gardenSelectedZone) renderGardenZoneDetail();
  }

  function escHtml(s) {
   const d = document.createElement('div');
   d.textContent = s;
   return d.innerHTML;
  }

  // Init reality tab
  renderTodos();
  renderNotes();
  renderGarden();

