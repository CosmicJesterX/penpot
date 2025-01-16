;; This Source Code Form is subject to the terms of the Mozilla Public
;; License, v. 2.0. If a copy of the MPL was not distributed with this
;; file, You can obtain one at http://mozilla.org/MPL/2.0/.
;;
;; Copyright (c) KALEIDOS INC

(ns app.main.ui.ds.product.avatar
  (:require-macros
   [app.common.data.macros :as dm]
   [app.main.style :as stl])
  (:require
   [app.main.ui.ds.foundations.typography :as t]
   [app.main.ui.ds.foundations.typography.text :refer [text*]]
   [rumext.v2 :as mf]))

(def ^:private schema:avatar
  [:map
   [:class {:optional true} :string]
   [:title :string]])

(mf/defc avatar*
  {::mf/props :obj
   ::mf/schema schema:avatar}
  [{:keys [class title children] :rest props}]

  (let [class (dm/str class " " (stl/css :cta))
        props (mf/spread-props props {:class class :data-testid "cta"})]
    [:> "div" props
     [:div {:class (stl/css :cta-title)}
      [:> text* {:as "span" :typography t/title-medium :class (stl/css :placeholder-title)} title]]
     [:div {:class (stl/css :cta-message)}
      children]]))
