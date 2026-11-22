// Moved out of web/pages/crafting.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
    (function(){
      'use strict';
      var recipes=[], categories=[], activeCat='all', query='';
      var grid=document.getElementById('recipe-grid');
      var countEl=document.getElementById('recipe-count');
      var emptyEl=document.getElementById('recipe-empty');
      var searchEl=document.getElementById('recipe-search');
      var filtersEl=document.getElementById('cat-filters');

      function esc(s){var d=document.createElement('div');d.textContent=(s==null?'':String(s));return d.innerHTML.replace(/"/g, '&quot;').replace(/'/g, '&#39;');}

      function itemsHtml(items,cls){
        if(!items||!items.length) return '<span class="io-items '+cls+'"><span class="io-item">'+esc(i18n.t('crafting.nothing'))+'</span></span>';
        return '<span class="io-items '+cls+'">'+items.map(function(it){
          return '<span class="io-item">'+esc(it.qty>1?i18n.t('crafting.item_count',{item:it.label,n:it.qty}):it.label)+'</span>';
        }).join('')+'</span>';
      }

      function card(r){
        var meta=[];
        if(r.station) meta.push(esc(i18n.t('crafting.station_meta',{station:r.station})));
        if(r.tools&&r.tools.length) meta.push(esc(i18n.t('crafting.tools_meta',{tools:r.tools.map(function(tool){return tool.at_home?i18n.t('crafting.tool_kept_at_home',{tool:tool.label}):tool.label;}).join(', ')})));
        if(r.skill) meta.push(esc(i18n.t('crafting.skill_meta',{skill:r.skill,level:r.skill_level})));
        if(r.craft_time_sec) meta.push(esc(i18n.t('crafting.seconds',{n:r.craft_time_sec})));
        // A dish (cooking rung 1): its method and safety line, in the desktop cooking card's words.
        var dishHtml='';
        if(r.dish){
          var lines=[i18n.t('cooking.method_line',{method:i18n.t('cooking.method_'+r.dish.method)})];
          var sf=r.dish.safe;
          if(sf){
            var line=sf.say?i18n.t(sf.say):(sf.core_c!=null?i18n.t('cooking.safe_core',{c:Math.round(sf.core_c),f:Math.round(sf.core_c*9/5+32)}):i18n.t('cooking.safe_boil',{n:sf.boil_min}));
            lines.push(i18n.t('cooking.hazard_serious',{line:line}));
          }
          dishHtml='<div class="recipe-dish">'+lines.map(function(l,i){return '<p class="'+(i?'recipe-hazard':'recipe-method')+'">'+esc(l)+'</p>';}).join('')+'</div>';
        }
        // A preserving procedure (preserving rung 1): what it must reach, and for canning the tested
        // table, in the desktop preserving card's words; the botulism hazard for low-acid food.
        if(r.preserve){
          var pv=r.preserve, pl=[];
          var band=function(list,unit){return (list||[]).map(function(b){var v=i18n.t(unit,{n:b.value});if(b.up_to_ft==null)return list.length===1?i18n.t('preserving.band_all',{v:v}):i18n.t('preserving.band_above',{v:v});return i18n.t('preserving.band_upto',{v:v,ft:b.up_to_ft.toLocaleString('en-US')});}).join(', ');};
          if(pv.kind==='dry'){
            pl.push(['recipe-method',i18n.t('preserving.method_dry')]);
            pl.push(['recipe-method',i18n.t(pv.class==='fruit'?'preserving.dry_until_fruit':'preserving.dry_until_vegetables')]);
          }else{
            pl.push(['recipe-method',i18n.t(pv.acid?'preserving.method_can_acid':'preserving.method_can_low')]);
            if(!pv.acid) pl.push(['recipe-danger',i18n.html('preserving.hazard_lethal'),true]);
            pv.tested.forEach(function(t){pl.push(['recipe-method',i18n.t('preserving.row_'+t.canner,{pints:band(t.pints,'preserving.minutes'),quarts:band(t.quarts,'preserving.minutes'),psi:band(t.psi,'preserving.pounds')})]);});
            pl.push(['recipe-method',i18n.t('preserving.half_pints')]);
          }
          dishHtml+='<div class="recipe-dish">'+pl.map(function(l){return '<p class="'+l[0]+'">'+(l[2]?l[1]:esc(l[1]))+'</p>';}).join('')+'</div>';
        }
        return '<div class="recipe-card">'+
          '<h3>'+esc(r.name)+'<span class="recipe-cat">'+esc(r.category)+'</span></h3>'+
          (r.description?'<p class="recipe-desc">'+esc(r.description)+'</p>':'')+
          '<div class="recipe-io">'+itemsHtml(r.inputs,'io-in')+
            '<span class="io-arrow" aria-label="'+esc(i18n.t('crafting.makes_label'))+'">→</span>'+
            itemsHtml(r.outputs,'io-out')+'</div>'+
          dishHtml+
          (meta.length?('<div class="recipe-meta">'+meta.map(function(m){return '<span>'+m+'</span>';}).join('')+'</div>'):'')+
        '</div>';
      }

      function render(){
        var q=query.trim().toLowerCase();
        var filtered=recipes.filter(function(r){
          if(activeCat!=='all' && r.category!==activeCat) return false;
          if(!q) return true;
          var hay=(r.name+' '+r.category+' '+r.station+' '+r.skill+' '+
            r.inputs.map(function(i){return i.label;}).join(' ')+' '+
            r.outputs.map(function(i){return i.label;}).join(' ')).toLowerCase();
          return hay.indexOf(q)!==-1;
        });
        grid.innerHTML=filtered.map(card).join('');
        emptyEl.hidden=filtered.length>0;
        countEl.textContent=i18n.t('crafting.count',{shown:filtered.length,total:recipes.length});
      }

      function buildFilters(){
        var cats=['all'].concat(categories);
        filtersEl.innerHTML=cats.map(function(c){
          var all=c==='all';
          var hintKey=all?'crafting.all_hint':'crafting.category_chip_hint';
          return '<button class="cat-chip" type="button" data-cat="'+esc(c)+'" aria-pressed="'+(c===activeCat)+'"'+
            ' data-i18n-tip="'+hintKey+'" data-tip-desc="'+esc(i18n.t(hintKey))+'">'+esc(all?i18n.t('crafting.all'):c)+'</button>';
        }).join('');
        Array.prototype.forEach.call(filtersEl.querySelectorAll('.cat-chip'),function(btn){
          btn.addEventListener('click',function(){
            activeCat=btn.dataset.cat;
            Array.prototype.forEach.call(filtersEl.querySelectorAll('.cat-chip'),function(b){
              b.setAttribute('aria-pressed', b.dataset.cat===activeCat ? 'true' : 'false');
            });
            render();
          });
        });
      }

      searchEl.addEventListener('input',function(){query=searchEl.value;render();});

      fetch('/data/recipes.json',{cache:'no-cache'}).then(function(r){return r.json();}).then(function(d){
        recipes=(d&&d.recipes)||[];
        categories=(d&&d.categories)||[];
        buildFilters();
        render();
      }).catch(function(){
        countEl.textContent='';
        grid.innerHTML='<div class="craft-empty">'+esc(i18n.t('crafting.load_failed'))+'</div>';
      });
    })();
