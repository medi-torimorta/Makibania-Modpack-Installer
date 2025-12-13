export type Language = "ja" | "en";

export interface Translation {
  alertOnLaunchModLoader: string;
  alertOnFailedAddProfile: string;
  alertOnFailedLaunchModLoader: string;
  appTitle: string;
  close: string;
  complete: string;
  error: string;
  install: string;
  installFailed: string;
  installerTitle: string;
  languageOptionEn: string;
  languageOptionJa: string;
  languageSelectionLabel: string;
  occurredError: string;
  openLogFolder: string;
  phaseAddProfile: string;
  phaseDownloadModLoader: string;
  phaseDownloadMods: string;
  phaseDownloadResources: string;
  phaseFinishInstall: string;
  phaseFinishUpdate: string;
  phaseLaunchModLoader: string;
  phaseRemoveMods: string;
  phaseStart: string;
  phaseUpdateSettings: string;
  titleMessage: string;
  update: string;
  updateFailed: string;
}

export const translations: Record<Language, Translation> = {
  ja: {
    alertOnLaunchModLoader: "Modローダーが起動します。'クライアントをインストール/Install Client'にチェックが入っていることを確認の上、続行してください。",
    alertOnFailedAddProfile: "プロファイルの追加に失敗しました。Minecraftランチャーを起動し、手動で追加してください。",
    alertOnFailedLaunchModLoader: "Modローダーの起動に失敗しました。ダウンロードされたModローダーを手動で実行してください。",
    appTitle: "Makibania Modpack Installer",
    close: "閉じる",
    complete: "完了",
    error: "エラー",
    install: "インストール",
    installFailed: "インストールに失敗しました。\n詳細: ",
    installerTitle: "インストーラー",
    languageOptionEn: "英語",
    languageOptionJa: "日本語",
    languageSelectionLabel: "表示言語",
    occurredError: "エラーが発生しました。\n詳細: ",
    openLogFolder: "ログフォルダを開く",
    phaseAddProfile: "プロファイルを追加中...",
    phaseDownloadModLoader: "Modローダーをダウンロード中...",
    phaseDownloadMods: "Modをダウンロード中...",
    phaseDownloadResources: "リソースをダウンロード中...",
    phaseFinishInstall: "インストールが完了しました。",
    phaseFinishUpdate: "アップデートが完了しました。",
    phaseLaunchModLoader: "Modローダーを起動中...",
    phaseRemoveMods: "不要なModを削除中...",
    phaseStart: "インストールを開始しています...",
    phaseUpdateSettings: "設定を更新中...",
    titleMessage: "実行するモードを選択してください。",
    update: "アップデート",
    updateFailed: "アップデートに失敗しました。\n詳細: ",
  },
  en: {
    alertOnLaunchModLoader: "The mod loader will be launched. Please ensure that 'Install client' is checked, then click 'Next'.",
    alertOnFailedAddProfile: "Failed to add profile. Please launch the Minecraft launcher and add it manually.",
    alertOnFailedLaunchModLoader: "Failed to launch mod loader. Please run the downloaded mod loader manually.",
    appTitle: "Makibania Modpack Installer",
    close: "Close",
    complete: "Complete",
    error: "Error",
    install: "Install",
    installFailed: "Installation failed.\nDetails: ",
    installerTitle: "Installer",
    languageOptionEn: "English",
    languageOptionJa: "Japanese",
    languageSelectionLabel: "Display language",
    occurredError: "An error has occurred.\nDetails: ",
    openLogFolder: "Open log folder",
    phaseAddProfile: "Adding profile...",
    phaseDownloadModLoader: "Downloading mod loader...",
    phaseDownloadMods: "Downloading mods...",
    phaseDownloadResources: "Downloading resources...",
    phaseLaunchModLoader: "Launching mod loader...",
    phaseFinishInstall: "Installation finished.",
    phaseFinishUpdate: "Update finished.",
    phaseRemoveMods: "Removing unnecessary mods...",
    phaseStart: "Starting installation...",
    phaseUpdateSettings: "Updating settings...",
    titleMessage: "Choose how you want to proceed.",
    update: "Update",
    updateFailed: "Update failed.\nDetails: ",
  },
};
