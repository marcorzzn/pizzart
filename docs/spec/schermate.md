# Schermate di Picsart — inventario derivato

> Documento **generato** da `tools/gen_spec.py` leggendo l'`AndroidManifest.xml`
> decodificato di `com.picsart.studio` 30.9.5 (`versionCode` 993830905).
> Non contiene codice né risorse dell'app: solo fatti strutturali (nomi di componenti,
> azioni, schemi di deeplink). Serve come capitolato per le schermate di pizzart.

## Numeri

| Componente | Conteggio |
|---|---:|
| Activity | 243 |
| di cui con deeplink | 13 |
| Service | 21 |
| Receiver | 21 |
| Content provider | 22 |
| Permessi dichiarati | 41 |
| Feature hardware | 10 |

## Aree funzionali

Le aree sono derivate dal nome del package della classe (regola deterministica,
definita in `AREE` dentro il generatore).

| Area | Activity | Da implementare in pizzart |
|---|---:|:---:|
| SDK terzi (fuori perimetro) | 50 | no |
| Abbonamento (fuori perimetro) | 34 | no |
| Account | 26 | sì |
| Pubblicità (fuori perimetro) | 24 | no |
| Altro | 23 | sì |
| Editor (superficie principale) | 22 | sì |
| Deeplink / ingresso | 9 | sì |
| Impostazioni | 9 | sì |
| Disegno | 7 | sì |
| Onboarding | 7 | sì |
| Video | 3 | sì |
| Oggetti (testo, sticker, forme) | 3 | sì |
| Libreria / importazione | 3 | sì |
| Social (fuori perimetro) | 3 | no |
| Creazione guidata | 3 | sì |
| Export / condivisione | 3 | sì |
| Telemetria (fuori perimetro) | 3 | no |
| Mini-app (fuori perimetro) | 2 | no |
| Progetti | 2 | sì |
| Web embedded | 1 | sì |
| Dialoghi | 1 | sì |
| Ricerca | 1 | sì |
| Fotocamera | 1 | sì |
| Effetti | 1 | sì |
| Ritaglio / formato | 1 | sì |
| Informazioni / dialoghi | 1 | sì |

**Assunzione di perimetro:** pizzart è un editor, non un social network.
Pubblicità, abbonamento, social, store e mini-app restano fuori: sono esattamente
il bloatware che il progetto vuole eliminare. L'assunzione è una riga di codice
(`FUORI_PERIMETRO` nel generatore) e si può ribaltare se serve.

Activity utili al perimetro: **127** su 243.

## Deeplink (ingressi esterni)

| Ingresso | Activity | Area |
|---|---|---|
| `lineauth:` | LineAuthenticationCallbackActivity | Account |
| `picsart://telekom/app` | DeutscheTelekomActivity | Altro |
| `https://picsart.com` | AuthActivity | Deeplink / ingresso |
| `picsart:`, `u`, `device-notification-settings` (+70) | DeepLinkActivity | Deeplink / ingresso |
| `picsart://instagram_oauth` | InstagramAuthActivity | Editor (superficie principale) |
| `picsartphotostudio:` | MainActivity | Editor (superficie principale) |
| `picsartphotostudio://oauth-success` | PreferencesActivity | Impostazioni |
| `db-qz0ubwmycfmwyj3:` | AuthActivity | SDK terzi (fuori perimetro) |
| `kakao1f43f170569b8e08b9fbdade822ddaf8://oauth` | AuthCodeHandlerActivity | SDK terzi (fuori perimetro) |
| `fbconnect://cct.com.picsart.studio` | CustomTabActivity | SDK terzi (fuori perimetro) |
| `picsart://pinterest_oauth` | PinterestAuthActivity | SDK terzi (fuori perimetro) |
| `picsart://snapchat` | SnapKitActivity | SDK terzi (fuori perimetro) |
| `picsart://pulse` | PAPreviewActivity | Telemetria (fuori perimetro) |

### Mappa delle rotte (`DeepLinkActivity`)

È il router universale dell'app: 73 combinazioni dichiarate,
schemi `http`, `https`, `picsart`, 0 percorsi distinti.
Questa è la tassonomia di navigazione da cui derivare le rotte di pizzart.

| # | percorso |
|---:|---|

## Activity per area (solo aree nel perimetro)

### Deeplink / ingresso (9)

| Schermata | Classe | Deeplink |
|---|---|---|
| AuthActivity | `com.picsart.jedi.presentation.deeplink` | `https://picsart.com` |
| CollageHandler | `com.picsart.editor.deeplink` | — |
| DeepLinkActivity | `com.picsart.deeplink` | `picsart:`, `u` (+71) |
| DeepLinkNativeActivity | `com.picsart.deeplink` | — |
| EditorHandler | `com.picsart.editor.deeplink` | — |
| FreestyleHandler | `com.picsart.editor.deeplink` | — |
| GridHandler | `com.picsart.editor.deeplink` | — |
| ReplayHandler | `com.picsart.editor.deeplink` | — |
| TemplateHandler | `com.picsart.editor.deeplink` | — |

### Web embedded (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| WebEditorMainActivity | `com.picsart.embedded.editor.impl` | — |

### Video (3)

| Schermata | Classe | Deeplink |
|---|---|---|
| BlooperActivity | `com.picsart.video.blooper` | — |
| VideoHandler | `com.picsart.video` | — |
| VideoNewActivity | `com.picsart.studio.editor.video` | — |

### Impostazioni (9)

| Schermata | Classe | Deeplink |
|---|---|---|
| AdsPreferenceActivity | `com.socialin.android.preference` | — |
| ContactSyncedActivity | `com.socialin.android.preference` | — |
| EditorPreferencesActivity | `com.socialin.android.preference.editor` | — |
| FBActionPreferencesActivity | `com.socialin.android.preference` | — |
| NotificationsPreferencesActivity | `com.picsart.notifications.settings` | — |
| OnBoardingPreferencesActivity | `com.socialin.android.preference` | — |
| PreferencesActivity | `com.socialin.android.preference` | `picsartphotostudio://oauth-success` |
| ShowSettingsActivity | `com.socialin.android.preference.settings` | — |
| SubscriptionPreferencesActivity | `com.socialin.android.preference` | — |

### Editor (superficie principale) (22)

| Schermata | Classe | Deeplink |
|---|---|---|
| AdPresentationActivity | `com.picsart.studio.ads.lib` | — |
| BlooperHandler | `com.socialin.android.photo.deeplinking` | — |
| EditProfileDetailsMiniAppActivity | `com.picsart.studio.profileminiapp.accountdetails` | — |
| EditorActivity | `com.picsart.studio.editor.main` | — |
| EditorAutoApplyActivity | `com.picsart.studio.editor.main` | — |
| ExceptionActivity | `com.picsart.studio.internal` | — |
| GifExportActivity | `com.picsart.studio.editor.tool.gif` | — |
| GoogleOauth2Activity | `com.picsart.studio.oauth2.google` | — |
| GoogleSignInActivity | `com.picsart.studio.google` | — |
| InfoDialogActivity | `com.picsart.studio.activity` | — |
| InstagramAuthActivity | `com.picsart.studio.oauth2` | `picsart://instagram_oauth` |
| MainActivity | `com.socialin.android.photo.picsinphoto` | `picsartphotostudio:` |
| MainPagerActivity | `com.socialin.android.photo.picsinphoto` | — |
| MiniAppHandler | `com.socialin.android.photo.deeplinking.miniapps` | — |
| ReplayWidgetConfig | `com.picsart.studio.widget` | — |
| ResizeDialogActivity | `com.socialin.android.photo.tools` | — |
| SimpleFeedActivity | `com.picsart.home.simplefeed` | — |
| SocialFeedActivity | `com.picsart.home` | — |
| StudioActivity | `com.socialin.android.photo.picsinphoto` | — |
| Text2ImageActivity | `com.picsart.studio.editor.tool.text2image` | — |
| VKAuthActivity | `com.picsart.studio.vkontakte` | — |
| VideoShareHandler | `com.socialin.android.photo.deeplinking` | — |

### Oggetti (testo, sticker, forme) (3)

| Schermata | Classe | Deeplink |
|---|---|---|
| AddEditCalloutActivity | `com.picsart.studio.editor.tools.addobjects.callout` | — |
| AddTextEditActivity | `com.picsart.studio.editor.tools.addobjects.text.ui.activities` | — |
| SelectLensFlareActivity | `com.socialin.android.photo.lensflare` | — |

### Disegno (7)

| Schermata | Classe | Deeplink |
|---|---|---|
| DrawBackgroundsActivity | `com.socialin.android.photo.draw.background` | — |
| DrawProjectSyncActivity | `com.picsart.draw` | — |
| DrawProjectsActivity | `com.socialin.android.photo.draw` | — |
| DrawingActivity | `com.socialin.android.photo.draw` | — |
| SelectClipArtFrameActivity | `com.socialin.android.photo.draw.textart` | — |
| SelectShapeDialogNew | `com.socialin.android.photo.draw.dialog` | — |
| VideoPreviewActivity | `com.socialin.android.photo.draw` | — |

### Dialoghi (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| CancelDialogActivity | `com.picsart.studio.dialog` | — |

### Libreria / importazione (3)

| Schermata | Classe | Deeplink |
|---|---|---|
| ChooserActivity | `com.picsart.chooser` | — |
| ChooserPreferencesActivity | `com.socialin.android.preference.chooser` | — |
| SizePresetsActivity | `com.picsart.chooser.sizepresets` | — |

### Altro (23)

| Schermata | Classe | Deeplink |
|---|---|---|
| AgeSharingConsentWrapperActivity | `com.google.android.play.agesignals` | — |
| CollectionActivity | `com.picsart.collections` | — |
| CreateCollectionActivity | `com.picsart.collections` | — |
| CreateOpenChatActivity | `com.linecorp.linesdk.openchat.ui` | — |
| DeutscheTelekomActivity | `com.picsart` | `picsart://telekom/app` |
| DiscoverPillDetailsActivity | `com.picsart.discovery.impl.ui.pills.details` | — |
| HexCodeInsertionActivity | `com.picsart.colorPicker` | — |
| HopMiniappLauncherActivity | `com.picsart.hop.impl.presentation` | — |
| IncentivisedInvitesThankYouActivity | `com.picsart.shopNew.activity` | — |
| PlayCoreDialogWrapperActivity | `com.google.android.play.core.common` | — |
| ProxyBillingActivity | `com.android.billingclient.api` | — |
| ProxyBillingActivityV2 | `com.android.billingclient.api` | — |
| ShakeMiniAppActivity | `com.picsart.shake` | — |
| ShopBrowseItemActivity | `com.picsart.shopNew.activity` | — |
| ShopItemListActivity | `com.picsart.shopNew.activity` | — |
| ShopItemPreviewDialogActivity | `com.picsart.shopNew.activity` | — |
| ShopTransparentActivity | `com.picsart.shopNew.activity` | — |
| SpacesMiniappActivity | `com.picsart.spaces.impl.presenter.spacespage` | — |
| SubscriptionContactUsActivity | `com.picsart.shopNew.activity` | — |
| SubscriptionInviteFriendsActivity | `com.picsart.shopNew.activity` | — |
| SubscriptionRegFlowFullScreenActivity | `com.picsart.shopNew.activity` | — |
| SubscriptionRestoreActivity | `com.picsart.shopNew.activity` | — |
| SubscriptionTransparentActivity | `com.picsart.shopNew.activity` | — |

### Account (26)

| Schermata | Classe | Deeplink |
|---|---|---|
| AllStickersActivity | `com.picsart.studio.picsart.profile.activity` | — |
| CommentImageZoomActivity | `com.picsart.studio.picsart.profile.activity` | — |
| CommentsActivity | `com.picsart.studio.picsart.profile.activity` | — |
| ContactsActivity | `com.picsart.studio.picsart.profile.activity` | — |
| DashboardActivity | `com.picsart.studio.profile.dashboard.activity` | — |
| DashboardTopsActivity | `com.picsart.studio.profile.dashboard.activity` | — |
| FindArtistsActivity | `com.picsart.studio.profile` | — |
| FindFriendsActivity | `com.picsart.studio.picsart.profile.activity` | — |
| GalleryItemShowActivity | `com.picsart.studio.picsart.profile.activity` | — |
| InviteFriendsAdditionalActivity | `com.picsart.studio.picsart.profile.activity` | — |
| InviteFriendsMainActivity | `com.picsart.studio.picsart.profile.activity` | — |
| LineAuthenticationActivity | `com.linecorp.linesdk.auth.internal` | — |
| LineAuthenticationCallbackActivity | `com.linecorp.linesdk.auth.internal` | `lineauth:` |
| PicsartAuthenticatorActivity | `com.picsart.auth.impl` | — |
| PrivacyActivity | `com.picsart.auth.impl.privacy.presenter.signin.view` | — |
| ProfileActivity | `com.picsart.studio.profile.view` | — |
| ProfileConnectionsActivity | `com.picsart.studio.picsart.profile.activity` | — |
| ProfileSettingsActivity | `com.picsart.studio.picsart.profile.activity` | — |
| RepliesActivity | `com.picsart.studio.picsart.profile.activity` | — |
| ReportMiniAppActivity | `com.picsart.profile.dialogs.imagereport` | — |
| SignInHubActivity | `com.google.android.gms.auth.api.signin.internal` | — |
| SimplePhotoActivity | `com.picsart.studio.picsart.profile.activity` | — |
| UserAvatarUploadActivity | `com.picsart.studio.picsart.profile.activity` | — |
| UserCollectionsActivity | `com.picsart.studio.profile.collections.activity` | — |
| UserListActivity | `com.picsart.studio.profile` | — |
| WebViewActivity | `com.picsart.studio.picsart.profile.activity` | — |

### Ricerca (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| SearchActivity | `com.picsart.search.ui` | — |

### Fotocamera (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| CameraResultHandlerActivity | `com.picsart.camera` | — |

### Effetti (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| StandaloneEffectActivity | `com.picsart.studio.editor.tool.effect` | — |

### Ritaglio / formato (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| StandaloneCropToolActivity | `com.picsart.studio.editor.tool.crop` | — |

### Creazione guidata (3)

| Schermata | Classe | Deeplink |
|---|---|---|
| CFAlbumChooserActivity | `com.picsart.createflow.dolphin3.presenter` | — |
| CreateFlowActivity | `com.picsart.createflow.view` | — |
| SelectPackageActivity | `com.picsart.create.selection.ui` | — |

### Onboarding (7)

| Schermata | Classe | Deeplink |
|---|---|---|
| FeatureOnboardingActivity | `com.picsart.growth.featureonboarding.impl` | — |
| HomeTutorialActivity | `com.picsart.home.tutorial` | — |
| IntentBasedOnboardingActivity | `com.picsart.growth.intentonboarding.impl.presentation` | — |
| LaunchOrderActivity | `com.picsart.growth.launchorder.impl` | — |
| OnboardIqActivity | `com.picsart.growth.onboardiq.impl.presentation` | — |
| SwipeVibeActivity | `com.picsart.growth.onboardiq.impl.swipeexperience` | — |
| TermsAndConditionsActivity | `com.picsart.growth.terms.presenter` | — |

### Informazioni / dialoghi (1)

| Schermata | Classe | Deeplink |
|---|---|---|
| DetectionInfoActivity | `com.picsart.detection.ui.info` | — |

### Export / condivisione (3)

| Schermata | Classe | Deeplink |
|---|---|---|
| ShareMiniAppActivity | `com.picsart.studio.share` | — |
| SharePlatformActivity | `com.picsart.studio.share` | — |
| SquareFitShareActivity | `com.picsart.studio.share.watermark` | — |

### Progetti (2)

| Schermata | Classe | Deeplink |
|---|---|---|
| SharedWithMeItemActivity | `com.picsart.userProjects.internal.cloudProject.commenting` | — |
| UserProjectsActivity | `com.picsart.userProjects.internal` | — |

## Componenti dichiarati ma non replicati

### Service

- `ActivityRecognitionManager` — SDK terzi (fuori perimetro)
- `AdService` — SDK terzi (fuori perimetro)
- `AppMeasurementJobService` — Altro
- `AppMeasurementService` — Altro
- `AppRestartDuringAdDetectionService` — SDK terzi (fuori perimetro)
- `AppseeBackgroundUploader` — Altro
- `AvatarUploadService` — Editor (superficie principale)
- `ComponentDiscoveryService` — SDK terzi (fuori perimetro)
- `FileDownloadService` — Altro
- `FirebaseMessagingService` — SDK terzi (fuori perimetro)
- `FullscreenAdService` — SDK terzi (fuori perimetro)
- `JobInfoSchedulerService` — Altro
- `MlKitComponentDiscoveryService` — Altro
- `MultiInstanceInvalidationService` — Altro
- `PAFirebaseInstanceIDService` — Altro
- `PAFirebaseMessagingService` — Altro
- `RevocationBoundService` — Account
- `SystemAlarmService` — Altro
- `SystemForegroundService` — Altro
- `SystemJobService` — Altro
- `TransportBackendDiscovery` — Altro

### Receiver

- `AlarmManagerSchedulerBroadcastReceiver` — Altro
- `AppMeasurementReceiver` — Altro
- `AuthenticationTokenManager$CurrentAuthenticationTokenChangedBroadcastReceiver` — SDK terzi (fuori perimetro)
- `CTPushNotificationReceiver` — SDK terzi (fuori perimetro)
- `ConstraintProxy$BatteryChargingProxy` — Altro
- `ConstraintProxy$BatteryNotLowProxy` — Altro
- `ConstraintProxy$NetworkStateProxy` — Altro
- `ConstraintProxy$StorageNotLowProxy` — Altro
- `ConstraintProxyUpdateReceiver` — Altro
- `CurrentAccessTokenExpirationBroadcastReceiver` — SDK terzi (fuori perimetro)
- `DiagnosticsReceiver` — Altro
- `FirebaseInstanceIdReceiver` — SDK terzi (fuori perimetro)
- `ForceStopRunnable$BroadcastReceiver` — Altro
- `NotificationBroadcastReceiver` — Altro
- `PTPushNotificationReceiver` — SDK terzi (fuori perimetro)
- `ProfileInstallReceiver` — Altro
- `PushTemplateReceiver` — SDK terzi (fuori perimetro)
- `ReplayWidgetProvider` — Editor (superficie principale)
- `RescheduleReceiver` — Altro
- `SnapCreativeShareResultHandler` — SDK terzi (fuori perimetro)
- `UploadNotificationBroadcastReceiver` — Progetti

### Content provider

- `AnalyticsFileProvider` — `com.picsart.studio.analytics.settings.provider`
- `AppLoadContentProvider` — `com.picsart.studio.AppLoadContentProvider`
- `AppLovinInitProvider` — `com.picsart.studio.applovininitprovider`
- `AudienceNetworkContentProvider` — `com.picsart.studio.AudienceNetworkContentProvider`
- `BugsnagContentProvider` — `com.picsart.studio.bugsnag-startup`
- `CleverTapFileProvider` — `com.picsart.studio.clevertap.fileprovider`
- `FacebookContentProvider` — `com.facebook.app.FacebookContentProvider108569252539536`
- `FacebookInitProvider` — `com.picsart.studio.FacebookInitProvider`
- `FileProvider` — `com.picsart.studio.fileprovider`
- `FirebaseInitProvider` — `com.picsart.studio.firebaseinitprovider`
- `InitializationProvider` — `com.picsart.studio.androidx-startup`
- `IronsourceLifecycleProvider` — `com.picsart.studio.IronsourceLifecycleProvider`
- `JediFileProvider` — `com.picsart.studio.jedi.fileprovider`
- `LevelPlayActivityLifecycleProvider` — `com.picsart.studio.LevelPlayActivityLifecycleProvider`
- `MlKitInitProvider` — `com.picsart.studio.mlkitinitprovider`
- `MobileAdsInitProvider` — `com.picsart.studio.mobileadsinitprovider`
- `PicassoProvider` — `com.picsart.studio.com.squareup.picasso`
- `SettingsPreConnectContentProvider` — `com.picsart.studio.analytics.SettingsPreConnectContentProvider`
- `SnapKitInitProvider` — `com.picsart.studio.SnapKitInitProvider`
- `SystemLifecycleContentProvider` — `com.picsart.studio.adjust-lifecycle-provider`
- `VKInitProvider` — `com.picsart.studio.VKInitProvider`
- `VungleProvider` — `com.picsart.studio.vungle-provider`

## Permessi: confronto con il target di pizzart

I permessi marcati *mai* sarebbero una regressione di privacy se li chiedessimo.

| Permesso | pizzart |
|---|---|
| `android.permission.READ_CONTACTS` | mai |
| `com.picsart.studio.permission.MAPS_RECEIVE` | solo se serve |
| `com.google.android.providers.gsf.permission.READ_GSERVICES` | solo se serve |
| `android.permission.INTERNET` | solo se serve |
| `android.permission.ACCESS_NETWORK_STATE` | solo se serve |
| `android.permission.WRITE_EXTERNAL_STORAGE` | solo se serve |
| `android.permission.READ_EXTERNAL_STORAGE` | solo se serve |
| `android.permission.READ_MEDIA_IMAGES` | solo se serve |
| `android.permission.READ_MEDIA_VIDEO` | solo se serve |
| `android.permission.READ_MEDIA_VISUAL_USER_SELECTED` | solo se serve |
| `android.permission.ACCESS_WIFI_STATE` | solo se serve |
| `com.google.android.gms.permission.ACTIVITY_RECOGNITION` | mai |
| `android.permission.RECORD_AUDIO` | solo se serve |
| `com.google.android.gms.permission.AD_ID` | mai |
| `android.permission.BIND_WALLPAPER` | solo se serve |
| `android.permission.CAMERA` | solo se serve |
| `com.android.vending.BILLING` | mai |
| `com.sec.android.iap.permission.BILLING` | mai |
| `android.permission.WAKE_LOCK` | solo se serve |
| `com.sec.android.provider.badge.permission.READ` | solo se serve |
| `com.sec.android.provider.badge.permission.WRITE` | solo se serve |
| `com.sonyericsson.home.permission.BROADCAST_BADGE` | solo se serve |
| `com.htc.launcher.permission.READ_SETTINGS` | solo se serve |
| `com.htc.launcher.permission.UPDATE_SHORTCUT` | solo se serve |
| `com.anddoes.launcher.permission.UPDATE_COUNT` | solo se serve |
| `android.permission.FOREGROUND_SERVICE_DATA_SYNC` | solo se serve |
| `android.permission.DETECT_SCREEN_CAPTURE` | mai |
| `android.permission.FOREGROUND_SERVICE` | solo se serve |
| `android.permission.VIBRATE` | solo se serve |
| `com.google.android.finsky.permission.BIND_GET_INSTALL_REFERRER_SERVICE` | solo se serve |
| `android.permission.POST_NOTIFICATIONS` | solo se serve |
| `com.android.vending.CHECK_LICENSE` | mai |
| `android.permission.ACCESS_ADSERVICES_ATTRIBUTION` | mai |
| `android.permission.ACCESS_ADSERVICES_AD_ID` | mai |
| `com.google.android.c2dm.permission.RECEIVE` | solo se serve |
| `android.permission.ACCESS_ADSERVICES_TOPICS` | mai |
| `android.permission.ACCESS_ADSERVICES_CUSTOM_AUDIENCE` | mai |
| `com.amazon.privacypass.ATTEST` | solo se serve |
| `android.permission.MODIFY_AUDIO_SETTINGS` | solo se serve |
| `com.picsart.studio.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION` | solo se serve |
| `android.permission.REORDER_TASKS` | solo se serve |

## Come si usa questo documento

- Una riga della tabella = una schermata che un utente Picsart si aspetta di trovare.
- Le aree diventano i moduli della UI (pannello sinistro, canvas, pannello destro).
- I deeplink diventano i comandi del command palette (stessa funzione, altra porta).
- La mappa delle rotte è la tassonomia di navigazione: da lì nascono i comandi.
- Le voci *fuori perimetro* non si implementano: sono il vantaggio competitivo.

