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
        if(r.tools&&r.tools.length) meta.push(esc(i18n.t('crafting.tools_meta',{tools:r.tools.map(function(tool){return tool.label;}).join(', ')})));
        if(r.skill) meta.push(esc(i18n.t('crafting.skill_meta',{skill:r.skill,level:r.skill_level})));
        if(r.craft_time_sec) meta.push(esc(i18n.t('crafting.seconds',{n:r.craft_time_sec})));
        return '<div class="recipe-card">'+
          '<h3>'+esc(r.name)+'<span class="recipe-cat">'+esc(r.category)+'</span></h3>'+
          (r.description?'<p class="recipe-desc">'+esc(r.description)+'</p>':'')+
          '<div class="recipe-io">'+itemsHtml(r.inputs,'io-in')+
            '<span class="io-arrow" aria-label="'+esc(i18n.t('crafting.makes_label'))+'">→</span>'+
            itemsHtml(r.outputs,'io-out')+'</div>'+
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
