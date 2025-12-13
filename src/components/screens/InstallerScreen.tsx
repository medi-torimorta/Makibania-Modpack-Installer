import ErrorIcon from "@mui/icons-material/Error";
import {
  Alert,
  AlertProps,
  Button,
  Container,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  LinearProgress,
  Stack,
  Typography,
} from "@mui/material";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { Translation } from "../../utils/localizer";

export const installerModes = ["install", "update"] as const;
export type InstallerMode = (typeof installerModes)[number];

type InstallerEvent =
  | {
      type: "changePhase";
      phase:
        | "downloadModLoader"
        | "removeMods"
        | "downloadMods"
        | "downloadResources"
        | "updateSettings"
        | "addProfile"
        | "launchModLoader";
    }
  | {
      type: "changeDetail";
      detail: string;
    }
  | {
      type: "updateProgress";
      progress: number;
    }
  | {
      type: "addAlert";
      level: "info" | "warning";
      translation_key: string;
    };

type AlertInfo = {
  level: AlertProps["severity"];
  message: string;
};

type InstallerScreenProps = {
  mode: InstallerMode;
  onComplete: () => void;
  onDismissError: () => void;
  translation: Translation;
};

export default function InstallerScreen(props: InstallerScreenProps) {
  const [phase, setPhase] = useState<string>(props.translation.phaseStart);
  const [detail, setDetail] = useState<string>("");
  const [progress, setProgress] = useState<number>(0);
  const [isFinished, setIsFinished] = useState<boolean>(false);
  const [alerts, setAlerts] = useState<AlertInfo[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const isInstalling = useRef(false);

  useEffect(() => {
    if (isInstalling.current) {
      return;
    }
    isInstalling.current = true;

    let unlisten: (() => void) | undefined;
    const runInstaller = async () => {
      unlisten = await listen<InstallerEvent>("installer://event", (event) => {
        const payload = event.payload;
        switch (payload.type) {
          case "changePhase":
            setDetail("");
            switch (payload.phase) {
              case "downloadModLoader":
                setPhase(props.translation.phaseDownloadModLoader);
                break;
              case "removeMods":
                setPhase(props.translation.phaseRemoveMods);
                break;
              case "downloadMods":
                setPhase(props.translation.phaseDownloadMods);
                break;
              case "downloadResources":
                setPhase(props.translation.phaseDownloadResources);
                break;
              case "updateSettings":
                setPhase(props.translation.phaseUpdateSettings);
                break;
              case "addProfile":
                setPhase(props.translation.phaseAddProfile);
                break;
              case "launchModLoader":
                setPhase(props.translation.phaseLaunchModLoader);
                break;
            }
            break;
          case "changeDetail":
            setDetail(payload.detail);
            break;
          case "updateProgress":
            setProgress(Math.round(payload.progress * 100));
            break;
          case "addAlert":
            setAlerts((alerts) => [
              ...alerts,
              {
                level: payload.level,
                message:
                  props.translation[
                    payload.translation_key as keyof Translation
                  ],
              },
            ]);
            break;
        }
      });
      try {
        await invoke("run_installer", { mode: props.mode });
        setPhase(
          props.mode === "install"
            ? props.translation.phaseFinishInstall
            : props.translation.phaseFinishUpdate
        );
        setDetail("");
        setIsFinished(true);
      } catch (e: unknown) {
        setErrorMessage(typeof e === "string" ? e : String(e));
      }
    };
    runInstaller();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [props.mode]);

  const handleClose = () => {
    setErrorMessage(null);
    props.onDismissError();
  };

  return (
    <Container
      maxWidth="sm"
      sx={{
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <Stack spacing={3} alignItems="center" sx={{ width: "100%" }}>
        <Typography variant="h6">{phase}</Typography>
        <Typography variant="subtitle2" color="text.secondary">
          {detail}
        </Typography>
        {!isFinished ? (
          <LinearProgress
            variant="determinate"
            value={progress}
            sx={{ width: "100%" }}
          />
        ) : (
          <Button variant="contained" onClick={props.onComplete}>
            {props.translation.complete}
          </Button>
        )}
        {alerts.map((alert, index) => (
          <Alert
            key={index}
            severity={alert.level}
            sx={{ width: "100%" }}
            onClose={() =>
              setAlerts((alerts) => alerts.filter((_, i) => i !== index))
            }
          >
            {alert.message}
          </Alert>
        ))}
      </Stack>
      <img
        src="/makiba-usagi.gif"
        alt="Makiba Usagi"
        style={{
          position: "absolute",
          bottom: 0,
          right: 0,
          width: "30vw",
          height: "30vw",
          maxWidth: "500px",
          maxHeight: "500px",
          pointerEvents: "none",
        }}
      />
      <Dialog
        open={errorMessage != null}
        aria-describedby="description"
        onClose={handleClose}
        sx={{ whiteSpace: "pre-line" }}
      >
        <DialogTitle sx={{ display: "flex", alignItems: "center" }}>
          <ErrorIcon color="error" sx={{ mr: 1 }} />
          {props.translation.error}
        </DialogTitle>
        <DialogContent>
          <DialogContentText id="description">
            {props.mode == "install"
              ? props.translation.installFailed
              : props.translation.updateFailed}
            {errorMessage}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => invoke("open_log_folder")}>
            {props.translation.openLogFolder}
          </Button>
          <Button onClick={handleClose} autoFocus>
            {props.translation.close}
          </Button>
        </DialogActions>
      </Dialog>
    </Container>
  );
}
